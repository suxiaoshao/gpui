export default function (pi) {
  pi.registerCommand("gupi-rpc-test", {
    description: "Isolated Gupi RPC protocol fixture; no model call",
    handler: async (_args, ctx) => {
      const selected = await ctx.ui.select("Select", ["first", "second"]);
      const confirmed = await ctx.ui.confirm("Confirm", "Continue?");
      const input = await ctx.ui.input("Input", "Placeholder");
      const edited = await ctx.ui.editor("Editor", "Original");
      ctx.ui.setStatus("fixture", "status");
      ctx.ui.setWidget("fixture", ["text"], { placement: "belowEditor" });
      ctx.ui.notify(JSON.stringify({ selected, confirmed, input, edited }), "info");
    },
  });
}
