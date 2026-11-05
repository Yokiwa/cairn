# 4. CurseForge API key（阶段 2 用）

**为什么需要：** 模组自动适配要去 CurseForge 查模组版本。CurseForge 的接口必须带一个 key 才能用。没有 key 时，Cairn 只用 Modrinth，CurseForge 的部分自动隐藏。

**要花的时间：** 5 分钟申请，通常当天就能拿到。

## 步骤

1. 打开 https://console.curseforge.com ，用 CurseForge（Overwolf）账号登录，没有就注册一个
2. 按提示填写开发者信息和项目说明：
   - 项目：Cairn，开源的 Minecraft Java 版启动器，https://github.com/Yokiwa/cairn
   - 用途：查询模组的版本、依赖，帮玩家下载模组
3. 在 **API Keys** 页面复制你的 key

## 把 key 交给构建（不要发在聊天里，也不要提交进代码）

1. 打开 https://github.com/Yokiwa/cairn/settings/secrets/actions
2. 点 **New repository secret**
   - **Name**：`CURSEFORGE_API_KEY`
   - **Secret**：粘贴 key
3. 让 Claude 在云端开发时也能用：Claude 会在需要时弹出一个安全表单，你把 key 填进表单（同样不要直接发在聊天里）

## 做完告诉 Claude
说一声"CurseForge key 已经加到 GitHub Secrets 了"就行。
