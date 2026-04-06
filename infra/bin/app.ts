#!/usr/bin/env node
import "source-map-support/register";
import * as cdk from "aws-cdk-lib";
import { TodoServerStack } from "../lib/todo-server-stack";

const app = new cdk.App();

new TodoServerStack(app, "TodoServerStack", {
  env: {
    account: process.env.CDK_DEFAULT_ACCOUNT,
    region: "ap-northeast-1",
  },
  description: "todo-server API on ECS Fargate with EFS-backed SQLite",
});
