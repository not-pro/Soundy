// Run on macOS: swift scripts/export-icons.swift
import AppKit

let icons = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    .deletingLastPathComponent().appendingPathComponent("assets/icons")

func image(_ name: String) -> NSImage {
    guard let image = NSImage(contentsOf: icons.appendingPathComponent(name)) else {
        fatalError("Cannot load \(name)")
    }
    return image
}

func png(_ image: NSImage, size: Int) -> Data {
    let bitmap = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size,
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
        colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0
    )!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
    NSGraphicsContext.current?.imageInterpolation = .high
    image.draw(in: NSRect(x: 0, y: 0, width: size, height: size),
               from: .zero, operation: .copy, fraction: 1)
    NSGraphicsContext.restoreGraphicsState()
    return bitmap.representation(using: .png, properties: [:])!
}

let app = image("soundy.png")
let tray = image("tray.svg")
try png(tray, size: 36).write(to: icons.appendingPathComponent("tray-template.png"))

let temporary = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
let iconset = temporary.appendingPathComponent("Soundy.iconset")
try FileManager.default.createDirectory(at: iconset, withIntermediateDirectories: true)
defer { try? FileManager.default.removeItem(at: temporary) }
for size in [16, 32, 128, 256, 512] {
    try png(app, size: size).write(to: iconset.appendingPathComponent("icon_\(size)x\(size).png"))
    try png(app, size: size * 2).write(to: iconset.appendingPathComponent("icon_\(size)x\(size)@2x.png"))
}
let process = Process()
process.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
process.arguments = ["-c", "icns", "-o", icons.appendingPathComponent("Soundy.icns").path, iconset.path]
try process.run()
process.waitUntilExit()
precondition(process.terminationStatus == 0, "iconutil failed")
print("Exported macOS icons to \(icons.path)")
