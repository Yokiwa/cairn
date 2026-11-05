# 3. Azure 应用（让启动器能用微软账号登录）

**为什么需要：** 第三方启动器要用自己的"应用 ID"去找微软登录，再由 Mojang 审核允许这个 ID 访问 Minecraft 的接口。没有它，Cairn 只能用离线账号（可以单人玩和测试，不能进正版服务器）。

**要花的时间：** 注册 10 分钟；Mojang 审核可能要几天到几周。

## 第一步：注册应用（免费）

1. 用你的微软账号登录 https://portal.azure.com
   - 第一次进可能要求创建一个免费的 Azure 账户或"目录"，按提示完成即可，**不需要付费订阅**
2. 顶部搜索框搜 **App registrations**（应用注册），点进去
3. 点 **New registration**（新注册）
   - **Name**：`Cairn`
   - **Supported account types**：选 **Personal Microsoft accounts only**（仅个人 Microsoft 帐户）
   - **Redirect URI**：留空
   - 点 **Register**
4. 在应用的 **Overview**（概述）页，复制 **Application (client) ID**（一串 `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`）。这个 ID 不是机密，可以公开
5. 左边点 **Authentication**（身份验证），拉到最下面 **Advanced settings**，把 **Allow public client flows**（允许公共客户端流）设为 **Yes**，点 **Save**

## 第二步：申请 Minecraft 接口权限

新注册的应用默认**不能**调用 Minecraft 的登录接口（会返回 403），要先过 Mojang 的审核：

1. 打开 Mojang 的第三方应用审核表单：https://aka.ms/mce-reviewappid
2. 填写：
   - 应用 ID：上一步复制的 Application (client) ID
   - 应用名称：Cairn
   - 用途：开源的 Minecraft Java 版第三方启动器，仓库 https://github.com/Yokiwa/cairn
3. 提交后等邮件通知

> 表单地址或流程如果变了，可以在 minecraft.wiki 的 "Microsoft authentication" 页面找最新的说明。

## 第三步：把 ID 交给构建

1. 打开 https://github.com/Yokiwa/cairn/settings/variables/actions
2. 点 **New repository variable**
   - **Name**：`CAIRN_MSA_CLIENT_ID`
   - **Value**：你的 Application (client) ID
3. 之后发布的版本里，"添加微软账号"就能用了

本地自己编译时，在命令前设置同名环境变量即可：`CAIRN_MSA_CLIENT_ID=你的ID`。

## 做完告诉 Claude
把 **Application (client) ID** 告诉 Claude（它不是密码，可以直接发），并说一下 Mojang 审核有没有通过。
