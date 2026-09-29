// Disposable native probe; not linked into Gupi or its settings.
import AppKit

final class Delegate: NSObject, NSApplicationDelegate {
    var window: NSWindow!
    var statusItem: NSStatusItem!
    let status = NSTextField(wrappingLabelWithString: "")
    let named = NSImage(named: "Color")
    var png: NSImage!

    func applicationDidFinishLaunching(_ notification: Notification) {
        let bundle = Bundle.main
        png = NSImage(contentsOf: bundle.url(forResource: "color", withExtension: "png")!)!
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 560, height: 310),
                          styleMask: [.titled, .closable], backing: .buffered, defer: false)
        window.title = "Gupi 图标验证（独立应用）"
        window.isReleasedWhenClosed = false
        let stack = NSStackView()
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 14
        stack.translatesAutoresizingMaskIntoConstraints = false
        window.contentView!.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: window.contentView!.leadingAnchor, constant: 24),
            stack.trailingAnchor.constraint(equalTo: window.contentView!.trailingAnchor, constant: -24),
            stack.topAnchor.constraint(equalTo: window.contentView!.topAnchor, constant: 24),
        ])
        stack.addArrangedSubview(NSTextField(labelWithString: "仅改变此验证程序，不修改 Gupi 或 Finder 图标"))
        for (label, action) in [("恢复打包的 .icon", #selector(reset)),
                                 ("切换到彩色 PNG", #selector(color)),
                                 ("切换到 Assets.car 中的 Color", #selector(compiled)),
                                 ("退出验证程序", #selector(quit))] {
            let button = NSButton(title: label, target: self, action: action)
            button.bezelStyle = .rounded
            if action == #selector(compiled) { button.isEnabled = named != nil }
            stack.addArrangedSubview(button)
        }
        stack.addArrangedSubview(status)
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        let tray = NSImage(contentsOf: bundle.url(forResource: "tray-template", withExtension: "png")!)!
        tray.size = NSSize(width: 18, height: 18)
        tray.isTemplate = true
        statusItem.button!.image = tray
        let menu = NSMenu()
        for (label, action) in [("彩色 Dock", #selector(color)), ("恢复默认", #selector(reset)), ("退出验证程序", #selector(quit))] {
            let item = NSMenuItem(title: label, action: action, keyEquivalent: "")
            item.target = self
            menu.addItem(item)
        }
        statusItem.menu = menu
        report("默认 .icon；NSImage(named: Color) = \(named == nil ? "nil" : "可加载")")
        window.center()
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        if CommandLine.arguments.contains("--smoke") {
            color()
            reset()
            DispatchQueue.main.asyncAfter(deadline: .now() + 1) { self.quit() }
        }
    }
    func report(_ text: String) {
        status.stringValue = text
        print(text)
        fflush(stdout)
    }
    @objc func color() {
        NSApp.applicationIconImage = png
        statusItem.button!.title = "1"
        NSApp.dockTile.badgeLabel = "1"
        report("彩色 PNG 已设置；Tray/Dock 请求显示数字 1（Dock 仍受系统授权影响）")
    }
    @objc func compiled() {
        guard let named else { return }
        NSApp.applicationIconImage = named
        report("已设置编译资源 Color；是否保留 Liquid Glass 需视觉检查")
    }
    @objc func reset() {
        NSApp.applicationIconImage = nil
        statusItem.button!.title = ""
        NSApp.dockTile.badgeLabel = nil
        report("已恢复打包的 .icon，并移除测试数字")
    }
    @objc func quit() { NSApp.terminate(nil) }
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
}
let app = NSApplication.shared
app.setActivationPolicy(.regular)
let delegate = Delegate()
app.delegate = delegate
app.run()
