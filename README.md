# ona_rust

コマンドラインで動作するTodoリスト管理ツールです。タスクの追加・完了・削除と、カテゴリによる分類をサポートします。データはホームディレクトリにJSONファイルとして保存されます。

## ビルド

```bash
cargo build --release
```

ビルド後のバイナリは `target/release/ona_rust` に生成されます。

## 使い方

```
todo <command> [args]
```

### コマンド一覧

| コマンド | 説明 |
|---|---|
| `add <text>` | Todoを追加する |
| `add --cat <category> <text>` | カテゴリ付きでTodoを追加する |
| `list` | すべてのTodoを表示する |
| `done <id>` | 指定したTodoを完了にする |
| `remove <id>` | 指定したTodoを削除する |
| `category add <name>` | カスタムカテゴリを追加する |
| `category list` | 利用可能なカテゴリを一覧表示する |

### 使用例

```bash
# Todoを追加
todo add 牛乳を買う

# カテゴリ付きで追加
todo add --cat shopping 牛乳を買う

# 一覧表示
todo list
# [ ] #1 [shopping]: 牛乳を買う

# 完了にする
todo done 1

# 削除する
todo remove 1

# カスタムカテゴリを追加
todo category add hobby

# カテゴリ一覧を表示
todo category list
```

## カテゴリ

組み込みカテゴリとして `work`、`personal`、`shopping`、`health` が用意されています。`category add` で任意のカテゴリを追加できます。

## データの保存先

| ファイル | 内容 |
|---|---|
| `~/.todos.json` | Todoリスト |
| `~/.todo_categories.json` | カスタムカテゴリ |
