# 5. 自动更新签名密钥（发布正式版本前）

**为什么需要：** Cairn 会自动检查 GitHub 上的新版本并更新。为了防止有人冒充你发布恶意更新，每个更新包都要用你的私钥签名，Cairn 用公钥验证。这对密钥是免费的，自己生成就行。

**没有它会怎样：** 安装包照样能打出来，只是"检查更新"按钮会隐藏，用户要手动下载新版本。

## 第一步：生成密钥（在你自己的电脑上）

装好 Node.js 22 和 pnpm 后，在仓库目录里运行：

```bash
pnpm -C ui install
```

```bash
pnpm -C ui tauri signer generate -w ~/.tauri/cairn.key
```

它会让你设一个密码（记住它），然后生成两个文件：
- `~/.tauri/cairn.key`：**私钥**，绝对不要公开、不要提交进代码、不要发给任何人
- `~/.tauri/cairn.key.pub`：**公钥**，可以公开

**一定要备份私钥和密码**（比如存进密码管理器）。丢了以后就再也发不出能被旧版本接受的更新了。

## 第二步：交给 GitHub

打开 https://github.com/Yokiwa/cairn/settings/secrets/actions ，添加两个 **Secret**：

| Name | 值 |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | `cairn.key` 文件的全部内容 |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 你设的密码 |

再打开 https://github.com/Yokiwa/cairn/settings/variables/actions ，添加一个 **Variable**：

| Name | 值 |
|---|---|
| `TAURI_UPDATER_PUBKEY` | `cairn.key.pub` 文件的全部内容 |

## 做完告诉 Claude
说一声"更新密钥已经配置好了"。之后发布的版本会自带自动更新。
