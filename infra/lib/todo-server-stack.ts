import * as cdk from "aws-cdk-lib";
import * as ec2 from "aws-cdk-lib/aws-ec2";
import * as ecr from "aws-cdk-lib/aws-ecr";
import * as ecs from "aws-cdk-lib/aws-ecs";
import * as efs from "aws-cdk-lib/aws-efs";
import * as elbv2 from "aws-cdk-lib/aws-elasticloadbalancingv2";
import * as iam from "aws-cdk-lib/aws-iam";
import * as logs from "aws-cdk-lib/aws-logs";
import { Construct } from "constructs";

export class TodoServerStack extends cdk.Stack {
  constructor(scope: Construct, id: string, props?: cdk.StackProps) {
    super(scope, id, props);

    // ── VPC ──────────────────────────────────────────────────────────────
    // 2 AZ, public + private subnets, single NAT gateway (cost-optimized)
    const vpc = new ec2.Vpc(this, "Vpc", {
      maxAzs: 2,
      natGateways: 1,
      subnetConfiguration: [
        {
          name: "public",
          subnetType: ec2.SubnetType.PUBLIC,
          cidrMask: 24,
        },
        {
          name: "private",
          subnetType: ec2.SubnetType.PRIVATE_WITH_EGRESS,
          cidrMask: 24,
        },
      ],
    });

    // ── ECR ──────────────────────────────────────────────────────────────
    const repository = new ecr.Repository(this, "Repository", {
      repositoryName: "todo-server",
      removalPolicy: cdk.RemovalPolicy.RETAIN,
      // Keep only the last 10 images to control storage costs
      lifecycleRules: [
        {
          maxImageCount: 10,
          description: "Keep last 10 images",
        },
      ],
    });

    // ── EFS ──────────────────────────────────────────────────────────────
    // SQLite DB file persisted across container restarts and deployments
    const fileSystem = new efs.FileSystem(this, "FileSystem", {
      vpc,
      vpcSubnets: { subnetType: ec2.SubnetType.PRIVATE_WITH_EGRESS },
      performanceMode: efs.PerformanceMode.GENERAL_PURPOSE,
      throughputMode: efs.ThroughputMode.BURSTING,
      removalPolicy: cdk.RemovalPolicy.RETAIN,
      encrypted: true,
    });

    const efsAccessPoint = new efs.AccessPoint(this, "EfsAccessPoint", {
      fileSystem,
      path: "/data",
      createAcl: {
        ownerGid: "1000",
        ownerUid: "1000",
        permissions: "755",
      },
      posixUser: {
        gid: "1000",
        uid: "1000",
      },
    });

    // ── ECS Cluster ───────────────────────────────────────────────────────
    const cluster = new ecs.Cluster(this, "Cluster", {
      vpc,
      clusterName: "todo-server",
      containerInsights: true,
    });

    // ── IAM: Task Execution Role ──────────────────────────────────────────
    const executionRole = new iam.Role(this, "TaskExecutionRole", {
      assumedBy: new iam.ServicePrincipal("ecs-tasks.amazonaws.com"),
      managedPolicies: [
        iam.ManagedPolicy.fromAwsManagedPolicyName(
          "service-role/AmazonECSTaskExecutionRolePolicy"
        ),
      ],
    });

    // ── IAM: Task Role (runtime permissions) ─────────────────────────────
    const taskRole = new iam.Role(this, "TaskRole", {
      assumedBy: new iam.ServicePrincipal("ecs-tasks.amazonaws.com"),
    });
    // Least-privilege EFS access: mount + write via the access point only.
    // grantRootAccess is intentionally avoided — the AccessPoint already
    // constrains the POSIX user to UID/GID 1000.
    fileSystem.grant(
      taskRole,
      "elasticfilesystem:ClientMount",
      "elasticfilesystem:ClientWrite"
    );

    // ── CloudWatch Log Group ──────────────────────────────────────────────
    const logGroup = new logs.LogGroup(this, "LogGroup", {
      logGroupName: "/ecs/todo-server",
      retention: logs.RetentionDays.ONE_MONTH,
      removalPolicy: cdk.RemovalPolicy.DESTROY,
    });

    // ── ECS Task Definition ───────────────────────────────────────────────
    const taskDefinition = new ecs.FargateTaskDefinition(
      this,
      "TaskDefinition",
      {
        memoryLimitMiB: 512,
        cpu: 256,
        executionRole,
        taskRole,
        // EFS volume mounted at /data inside the container
        volumes: [
          {
            name: "efs-data",
            efsVolumeConfiguration: {
              fileSystemId: fileSystem.fileSystemId,
              transitEncryption: "ENABLED",
              authorizationConfig: {
                accessPointId: efsAccessPoint.accessPointId,
                iam: "ENABLED",
              },
            },
          },
        ],
      }
    );

    const container = taskDefinition.addContainer("todo-server", {
      // Placeholder image — GitHub Actions will update this on first deploy
      image: ecs.ContainerImage.fromRegistry("public.ecr.aws/amazonlinux/amazonlinux:latest"),
      portMappings: [{ containerPort: 3000 }],
      environment: {
        PORT: "3000",
        HOME: "/data",
      },
      logging: ecs.LogDrivers.awsLogs({
        logGroup,
        streamPrefix: "todo-server",
      }),
      healthCheck: {
        command: [
          "CMD-SHELL",
          "wget -qO- http://localhost:3000/categories || exit 1",
        ],
        interval: cdk.Duration.seconds(30),
        timeout: cdk.Duration.seconds(5),
        retries: 3,
        startPeriod: cdk.Duration.seconds(10),
      },
    });

    container.addMountPoints({
      containerPath: "/data",
      sourceVolume: "efs-data",
      readOnly: false,
    });

    // ── Security Groups ───────────────────────────────────────────────────
    const albSg = new ec2.SecurityGroup(this, "AlbSg", {
      vpc,
      description: "ALB inbound HTTP",
      allowAllOutbound: true,
    });
    albSg.addIngressRule(ec2.Peer.anyIpv4(), ec2.Port.tcp(80));
    albSg.addIngressRule(ec2.Peer.anyIpv6(), ec2.Port.tcp(80));

    const serviceSg = new ec2.SecurityGroup(this, "ServiceSg", {
      vpc,
      description: "ECS Fargate service",
      allowAllOutbound: true,
    });
    // Only accept traffic from ALB
    serviceSg.addIngressRule(albSg, ec2.Port.tcp(3000));

    // EFS allows inbound NFS from the service
    fileSystem.connections.allowDefaultPortFrom(serviceSg);

    // ── ALB ───────────────────────────────────────────────────────────────
    const alb = new elbv2.ApplicationLoadBalancer(this, "Alb", {
      vpc,
      internetFacing: true,
      securityGroup: albSg,
      vpcSubnets: { subnetType: ec2.SubnetType.PUBLIC },
    });

    const targetGroup = new elbv2.ApplicationTargetGroup(this, "TargetGroup", {
      vpc,
      port: 3000,
      protocol: elbv2.ApplicationProtocol.HTTP,
      targetType: elbv2.TargetType.IP,
      healthCheck: {
        path: "/categories",
        healthyHttpCodes: "200",
        interval: cdk.Duration.seconds(30),
        timeout: cdk.Duration.seconds(5),
        healthyThresholdCount: 2,
        unhealthyThresholdCount: 3,
      },
      deregistrationDelay: cdk.Duration.seconds(30),
    });

    alb.addListener("HttpListener", {
      port: 80,
      defaultTargetGroups: [targetGroup],
    });

    // ── ECS Fargate Service ───────────────────────────────────────────────
    const service = new ecs.FargateService(this, "Service", {
      cluster,
      taskDefinition,
      desiredCount: 1,
      securityGroups: [serviceSg],
      vpcSubnets: { subnetType: ec2.SubnetType.PRIVATE_WITH_EGRESS },
      assignPublicIp: false,
      // Rolling update: keep at least 1 task running during deployment
      minHealthyPercent: 100,
      maxHealthyPercent: 200,
      circuitBreaker: { rollback: true },
      // Off by default. Enable with: cdk deploy -c enableExec=true
      enableExecuteCommand:
        this.node.tryGetContext("enableExec") === "true",
    });

    service.attachToApplicationTargetGroup(targetGroup);

    // ── Outputs ───────────────────────────────────────────────────────────
    new cdk.CfnOutput(this, "ApiUrl", {
      value: `http://${alb.loadBalancerDnsName}`,
      description: "API endpoint URL",
      exportName: "TodoServerApiUrl",
    });

    new cdk.CfnOutput(this, "EcrRepositoryUri", {
      value: repository.repositoryUri,
      description: "ECR repository URI for GitHub Actions",
      exportName: "TodoServerEcrUri",
    });

    new cdk.CfnOutput(this, "EcsClusterName", {
      value: cluster.clusterName,
      description: "ECS cluster name for GitHub Actions",
      exportName: "TodoServerClusterName",
    });

    new cdk.CfnOutput(this, "EcsServiceName", {
      value: service.serviceName,
      description: "ECS service name for GitHub Actions",
      exportName: "TodoServerServiceName",
    });

    new cdk.CfnOutput(this, "TaskDefinitionFamily", {
      value: taskDefinition.family,
      description: "ECS task definition family for GitHub Actions",
      exportName: "TodoServerTaskDefFamily",
    });
  }
}
