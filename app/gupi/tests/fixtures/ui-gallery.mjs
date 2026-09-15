// Loaded explicitly by the isolated gallery launcher, never installed globally.
const scenes = {
  select: "单选 · 三个选项",
  "select-long": "单选 · 长文本与 24 个选项",
  confirm: "确认 · 确认、否、取消",
  input: "输入 · 占位文字与中文",
  editor: "编辑器 · 多行预填",
  sequence: "问卷 · 连续四步交互",
  notify: "通知 · 三种级别",
  status: "状态 · 多项与更新",
  "status-clear": "状态 · 清除",
  "widget-above": "Widget · 输入区上方",
  "widget-below": "Widget · 输入区下方",
  "widget-clear": "Widget · 清除",
  title: "标题 · 更新扩展提示",
  "editor-text": "草稿 · 替换输入文字",
  timeout: "超时 · 选择、确认、输入各 8 秒",
  delayed: "延迟 · 5 秒后提问，可先切换会话",
};
const prefill = "# 测试草稿\n\n这是一段可编辑的中文。请尝试选中、换行和输入法组词。\n\n```rust\nfn main() {\n    println!(\"Hello, Gupi!\");\n}\n```\n\n最后一段：提交后检查下方收到的文本。";

export default function (pi) {
  let revision = 0;
  const timers = new Set();
  const result = (ctx, scene, value) => {
    const text = JSON.stringify({ scene, value: value ?? null }, null, 2);
    ctx.ui.setWidget("gupi-gallery-result", ["最近一次交互结果", ...text.split("\n")], {
      placement: "belowEditor",
    });
    ctx.ui.notify(`${scene} 已结束，结果见输入区下方`, "info");
  };
  pi.on("session_start", async (_event, ctx) => {
    ctx.ui.setWidget("gupi-gallery-help", [
      "临时插件 UI 体验 · 无模型调用",
      "输入 /gupi-ui 打开菜单；或 /gupi-ui select、confirm、input、editor 直接体验。",
      "试试 /gupi-ui sequence 连续问答。这里只使用临时会话和配置。",
    ], { placement: "aboveEditor" });
  });
  pi.on("session_shutdown", async () => {
    for (const timer of timers) clearTimeout(timer);
    timers.clear();
  });
  pi.registerCommand("gupi-ui", {
    description: "临时插件 UI 菜单；不调用模型",
    handler: async (args, ctx) => {
      let scene = args.trim();
      if (!scene) {
        const labels = Object.entries(scenes).map(([key, label]) => `${key} · ${label}`);
        const selected = await ctx.ui.select("插件 UI 体验", labels);
        if (selected === undefined) return;
        scene = selected.split(" · ")[0];
      }
      if (!(scene in scenes)) {
        ctx.ui.notify("未知场景，请输入 /gupi-ui", "warning");
        return;
      }
      switch (scene) {
        case "select":
          result(ctx, scene, await ctx.ui.select("你想怎样处理这段文字？", ["翻译成中文", "润色表达", "解释术语"]));
          break;
        case "select-long":
          result(ctx, scene, await ctx.ui.select("长列表：观察窗口高度、选项换行与滚动", Array.from({ length: 24 }, (_, i) => `${i + 1}. ${i % 3 === 0 ? "这是一个很长的选项，用来观察中英文混排、内容截断与换行是否容易阅读 — UI layout and scrolling" : "测试选项：检查鼠标与键盘操作"}`)));
          break;
        case "confirm":
          result(ctx, scene, await ctx.ui.confirm("是否继续测试？", "这是一条演示说明。确认、否和取消都不会执行真实任务。\n请观察按钮位置、焦点和退出后的输入恢复。"));
          break;
        case "input":
          result(ctx, scene, await ctx.ui.input("为测试项目起个名字", "例如：中文输入法测试"));
          break;
        case "editor":
          result(ctx, scene, await ctx.ui.editor("编辑测试草稿", prefill));
          break;
        case "sequence": {
          const values = {};
          values.selected = await ctx.ui.select("1/4 · 选择语言", ["中文", "English", "日本語"]);
          if (values.selected === undefined) { result(ctx, scene, values); break; }
          values.confirmed = await ctx.ui.confirm("2/4 · 确认选择", `已选择 ${values.selected}，继续填写？`);
          if (!values.confirmed) { result(ctx, scene, values); break; }
          values.input = await ctx.ui.input("3/4 · 输入主题", "可输入中文或留空");
          if (values.input === undefined) { result(ctx, scene, values); break; }
          values.edited = await ctx.ui.editor("4/4 · 编辑最终说明", `语言：${values.selected}\n主题：${values.input}\n\n补充说明：`);
          result(ctx, scene, values);
          break;
        }
        case "notify":
          for (const type of ["info", "warning", "error"]) ctx.ui.notify(`${type}：这是一条演示通知，不代表实际故障。`, type);
          break;
        case "status":
          ctx.ui.setStatus("gupi-gallery-state", `测试状态 · 第 ${++revision} 次更新`);
          ctx.ui.setStatus("gupi-gallery-hint", "再次运行 status 更新同一项；status-clear 清除");
          break;
        case "status-clear":
          ctx.ui.setStatus("gupi-gallery-state", undefined);
          ctx.ui.setStatus("gupi-gallery-hint", undefined);
          break;
        case "widget-above":
        case "widget-below": {
          const above = scene === "widget-above";
          ctx.ui.setWidget(`gupi-gallery-${above ? "above" : "below"}`, [
            `${above ? "上方" : "下方"}内容 · 第 ${++revision} 次更新`,
            "第一行：插件可以展示任务进度或操作提示。",
            "第二行：这里只接收文本数组，没有任意 TUI 组件。",
          ], { placement: above ? "aboveEditor" : "belowEditor" });
          break;
        }
        case "widget-clear":
          for (const key of ["above", "below", "result", "help"]) ctx.ui.setWidget(`gupi-gallery-${key}`, undefined);
          break;
        case "title":
          ctx.ui.setTitle(`插件 UI 体验 · ${++revision}`);
          break;
        case "editor-text":
          ctx.ui.setEditorText("这是插件回填的普通草稿，可以继续编辑；不是模型消息。");
          break;
        case "timeout": {
          const values = {};
          values.selected = await ctx.ui.select("1/3 · 8 秒后自动结束", ["也可以提前选择"], { timeout: 8000 });
          values.confirmed = await ctx.ui.confirm("2/3 · 8 秒后自动结束", "不操作即可观察超时", { timeout: 8000 });
          values.input = await ctx.ui.input("3/3 · 8 秒后自动结束", "可以提前输入或取消", { timeout: 8000 });
          result(ctx, scene, values);
          break;
        }
        case "delayed": {
          ctx.ui.notify("5 秒后在来源会话提问；现在可输入草稿或切换会话。", "info");
          const timer = setTimeout(async () => {
            timers.delete(timer);
            try {
              result(ctx, scene, await ctx.ui.select("延迟提问 · 检查来源会话", ["答案 A", "答案 B"]));
            } catch (error) {
              console.error("gallery delayed dialog:", error);
            }
          }, 5000);
          timers.add(timer);
          break;
        }
      }
    },
  });
  // Pi dispatches registered commands first; every other prompt stops here.
  pi.on("input", async (_event, ctx) => {
    ctx.ui.notify("此环境只演示 UI，请输入 /gupi-ui；普通文本不会发给模型。", "info");
    return { action: "handled" };
  });
}
