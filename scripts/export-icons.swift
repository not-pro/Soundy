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

// ICO directory entries point to PNG payloads (supported since Windows Vista).
func ico(_ image: NSImage, sizes: [Int]) -> Data {
    let frames = sizes.map { png(image, size: $0) }
    var result = Data()
    func word(_ value: Int, bytes: Int = 2) {
        for byte in 0..<bytes { result.append(UInt8((value >> (byte * 8)) & 255)) }
    }
    word(0); word(1); word(sizes.count)
    var offset = 6 + sizes.count * 16
    for (size, frame) in zip(sizes, frames) {
        word(size == 256 ? 0 : size, bytes: 1)
        word(size == 256 ? 0 : size, bytes: 1)
        word(0); word(1); word(32)
        word(frame.count, bytes: 4); word(offset, bytes: 4)
        offset += frame.count
    }
    for frame in frames { result.append(frame) }
    return result
}

let app = image("soundy.png")
let tray = image("tray.svg")
try png(tray, size: 36).write(to: icons.appendingPathComponent("tray-template.png"))
try ico(app, sizes: [16, 20, 24, 32, 40, 48, 64, 128, 256])
    .write(to: icons.appendingPathComponent("Soundy.ico"))
// Solid amber strokes retain contrast in either Windows taskbar theme.
let windowsTray = NSImage(data: Data(
    try String(contentsOf: icons.appendingPathComponent("tray.svg"), encoding: .utf8)
        .replacingOccurrences(of: "#000", with: "#C66318").utf8
))!
try png(windowsTray, size: 32).write(to: icons.appendingPathComponent("tray-windows.png"))
try ico(windowsTray, sizes: [16, 20, 24, 32, 40, 48, 64])
    .write(to: icons.appendingPathComponent("tray-windows.ico"))

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
print("Exported macOS and Windows icons to \(icons.path)")
