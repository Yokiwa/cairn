# 1. GitHub 仓库 ✅

**状态：** 已完成（仓库 `Yokiwa/cairn`，公开）。

## 以后会用到的操作

### 合并开发分支
Claude 在 `phase1-playable` 这样的分支上开发。要合并时：
1. 打开 https://github.com/Yokiwa/cairn/pulls ，点 Claude 开的 Pull Request（或者点 **Compare & pull request** 自己开一个）
2. 等页面下方的检查（CI）全部变绿
3. 点 **Merge pull request** → **Confirm merge**

### 发布一个版本
1. 在 `main` 上推一个标签，比如 `v0.1.0`（可以让 Claude 帮你推）
2. 打开 https://github.com/Yokiwa/cairn/actions ，等 **Release** 工作流跑完（约 20–30 分钟）
3. 打开 https://github.com/Yokiwa/cairn/releases ，会看到一个**草稿**版本，里面有 Windows、macOS、Linux 的安装包
4. 检查无误后点 **Edit** → **Publish release**

## 做完告诉 Claude
不用，这一项已经完成。
