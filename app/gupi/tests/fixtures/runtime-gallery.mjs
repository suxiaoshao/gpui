// Explicitly loaded by script/gupi-runtime-gallery; never installed globally.
import { mkdirSync, realpathSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { Type, fauxProvider, fauxAssistantMessage, fauxThinking, fauxText, fauxToolCall } from "@earendil-works/pi-ai";
import {
  createReadToolDefinition, createWriteToolDefinition, createEditToolDefinition,
  createBashToolDefinition, createGrepToolDefinition, createFindToolDefinition,
  createLsToolDefinition,
} from "@earendil-works/pi-coding-agent";

export default function (pi) {
  if (!process.env.GUPI_RUNTIME_PROJECT) throw new Error("Use script/gupi-runtime-gallery to load this fixture.");
  const project = realpathSync(process.env.GUPI_RUNTIME_PROJECT);
  const provider = "gupi-runtime";
  const faux = fauxProvider({
    provider,
    models: [{ id: "gallery", name: "Runtime Gallery · 慢速运行", reasoning: true }],
    tokensPerSecond: 2,
    tokenSize: { min: 1, max: 1 },
  });
  pi.registerProvider(faux.provider);

  const result = (text) => ({ content: [{ type: "text", text }], details: {} });
  const assertProject = (ctx) => {
    if (realpathSync(ctx.cwd) !== project || ctx.model?.provider !== provider) {
      throw new Error("This fixture only runs in its temporary project with Runtime Gallery selected.");
    }
  };
  // Reuse the real tools. Delays make their running markers visible in Gupi.
  for (const create of [createReadToolDefinition, createWriteToolDefinition, createEditToolDefinition,
    createBashToolDefinition, createGrepToolDefinition, createFindToolDefinition, createLsToolDefinition]) {
    const tool = create(project);
    pi.registerTool({
      ...tool,
      async execute(id, args, signal, onUpdate, ctx) {
        assertProject(ctx);
        await delay(2500, undefined, { signal });
        return tool.execute(id, args, signal, onUpdate, ctx);
      },
    });
  }
  pi.registerTool({
    name: "runtime_check",
    label: "Runtime check",
    description: "A temporary tool that reports three progress updates without external requests.",
    parameters: Type.Object({}),
    async execute(_id, _args, signal, onUpdate, ctx) {
      assertProject(ctx);
      for (const text of ["1/3 检查测试文件", "2/3 检查结果展示", "3/3 检查完成"]) {
        onUpdate?.(result(text));
        await delay(2000, undefined, { signal });
      }
      return result("测试检查完成，三次进度更新已发送。");
    },
  });

  pi.on("before_agent_start", (_event, ctx) => {
    if (ctx.model?.provider !== provider) return;
    assertProject(ctx);
    mkdirSync(join(project, "skills", "review"), { recursive: true });
    mkdirSync(join(project, "skills", "writing"), { recursive: true });
    writeFileSync(join(project, "skills", "review", "SKILL.md"),
      "---\nname: gallery-review\ndescription: 临时检查技能\n---\n\n# 检查步骤\n\n先列目录，再读取说明，最后检查 TODO。\n");
    writeFileSync(join(project, "skills", "writing", "SKILL.md"),
      "---\nname: gallery-writing\ndescription: 临时写作技能\n---\n\n# 输出规则\n\n过程说明与最终回答分开。报告包含完成项与验证结果。\n");
    writeFileSync(join(project, "notes.md"), "# 测试项目\n\nTODO: 检查消息顺序。\nTODO: 观察运行图标。\n");

    const thinking = (text) => fauxThinking(text);
    const text = (value) => fauxText(value);
    const call = (name, args) => fauxToolCall(name, args);
    const step = (...blocks) => () => fauxAssistantMessage(blocks, { stopReason: "toolUse" });
    faux.setResponses([
      step(
        thinking("我先梳理这个临时项目的内容。这里故意慢慢思考，方便观察思考图标、运行提示和内容展开。先看目录，再读取第一份技能说明，之后才开始处理文件。"),
        text("我先查看目录和检查技能，再定位待处理内容。"),
        call("ls", { path: "." }),
      ),
      step(call("read", { path: "skills/review/SKILL.md" })),
      step(
        thinking("已经读到检查规则。接下来查找 Markdown 文件和待办标记，确认要修改的范围。"),
        call("find", { pattern: "*.md", path: "." }),
        call("grep", { pattern: "TODO", path: "notes.md" }),
      ),
      step(
        text("已找到两条待办。我会写入初稿，再做一次局部修改；这仍是过程说明。"),
        call("write", { path: "report.md", content: "# 运行展示报告\n\n状态：初稿\n\n- 目录已检查\n- 第一份 Skill 已读取\n" }),
      ),
      step(call("edit", { path: "report.md", edits: [{ oldText: "状态：初稿", newText: "状态：第一轮检查完成" }] })),
      step(call("bash", { command: "printf '第一轮工具调用完成\\n'; wc -l notes.md report.md" })),
      step(call("runtime_check", {})),
      step(
        text("第一轮处理已经完成。接下来读取另一份技能，再补充报告并复查内容。"),
        thinking("现在进入第二轮。上一轮的工具已经结束，但整个任务还没有完成。这里再次慢速思考，让你观察旧的过程是否折叠、新的思考是否出现，以及运行提示有没有过早消失。"),
        call("read", { path: "skills/writing/SKILL.md" }),
      ),
      step(
        thinking("第二份技能要求明确区分过程与结论。我先读回报告，确认第一轮编辑已生效，再追加第二轮结果。"),
        call("read", { path: "report.md" }),
      ),
      step(
        text("报告中的第一轮修改已生效。我正在补充第二轮内容，随后做最后检查。"),
        call("edit", { path: "report.md", edits: [{ oldText: "状态：第一轮检查完成", newText: "状态：全部完成\n\n- 第二份 Skill 已读取\n- 第二轮复查完成" }] }),
      ),
      step(call("bash", { command: "printf '第二轮复查\\n'; cat report.md" })),
      step(call("runtime_check", {})),
      () => fauxAssistantMessage([
        thinking("两轮思考、技能读取和工具调用均已完成。现在结束过程，给出最终回答。"),
        text("## 最终回答\n\n本轮展示已完成。\n\n| 类型 | 内容 |\n| --- | --- |\n| 思考 | 两轮慢速输出 |\n| 读取 | 两份 SKILL.md 与报告 |\n| 搜索 | ls、find、grep |\n| 修改 | write、edit |\n| 终端 | 两次 bash |\n| 通用工具 | 两次进度检查 |\n\n中间的说明属于过程消息；这一条才是最终回答。你可以打开会话历史，对比简略、详细、全部视图。\n\n再发送任意文字即可重播；也可以在下一轮中途停止或切换会话。"),
      ]),
    ]);
  });
}
