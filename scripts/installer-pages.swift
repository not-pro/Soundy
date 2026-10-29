// Generate native Installer pages with embedded images (no external HTML assets).
import AppKit

let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
let destination = URL(fileURLWithPath: CommandLine.arguments[1])
let iconURL = root.appendingPathComponent("assets/icons/soundy.png")
let amber = NSColor(srgbRed: 0.65, green: 0.29, blue: 0.07, alpha: 1)
let ink = NSColor(srgbRed: 0.25, green: 0.20, blue: 0.17, alpha: 1)

func page(_ name: String, title: String, subtitle: String, detail: String) throws {
    let result = NSMutableAttributedString(string: "")
    let centered = NSMutableParagraphStyle()
    centered.alignment = .center
    centered.lineSpacing = 2
    centered.paragraphSpacing = 6
    func text(_ value: String, size: CGFloat, weight: NSFont.Weight = .regular, color: NSColor = ink) {
        result.append(NSAttributedString(string: value + "\n", attributes: [
            .font: NSFont.systemFont(ofSize: size, weight: weight),
            .foregroundColor: color, .paragraphStyle: centered,
        ]))
    }
    let image = NSImage(contentsOf: iconURL)!
    image.size = NSSize(width: 64, height: 64)
    // RTFD readers use the image's intrinsic size, ignoring RTF width/height.
    // TIFF keeps a 64 pt logical size with 128 px Retina detail on round-trip.
    let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 128, pixelsHigh: 128,
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
        colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    bitmap.size = image.size
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
    image.draw(in: NSRect(origin: .zero, size: image.size), from: .zero, operation: .copy, fraction: 1)
    NSGraphicsContext.restoreGraphicsState()
    let imageData = bitmap.representation(using: .tiff,
        properties: [.compressionMethod: NSBitmapImageRep.TIFFCompression.lzw.rawValue])!
    let wrapper = FileWrapper(regularFileWithContents: imageData)
    wrapper.preferredFilename = "soundy.tiff"
    let attachment = NSTextAttachment(fileWrapper: wrapper)
    attachment.attachmentCell = NSTextAttachmentCell(imageCell: NSImage(data: imageData)!)
    let logo = NSMutableAttributedString(attachment: attachment)
    logo.addAttribute(.paragraphStyle, value: centered, range: NSRange(location: 0, length: logo.length))
    result.append(logo)
    text("", size: 3)
    text(title, size: 26, weight: .semibold, color: amber)
    text(subtitle, size: 13, weight: .medium)
    text(detail, size: 12)
    let document = try result.fileWrapper(from: NSRange(location: 0, length: result.length),
        documentAttributes: [.documentType: NSAttributedString.DocumentType.rtfd,
                             .paperSize: NSSize(width: 410, height: 330)])
    try document.write(to: destination.appendingPathComponent(name + ".rtfd"), options: .atomic, originalContentsURL: nil)
    let decoded = try NSAttributedString(url: destination.appendingPathComponent(name + ".rtfd"),
        options: [.documentType: NSAttributedString.DocumentType.rtfd], documentAttributes: nil)
    let decodedIcon = decoded.attribute(.attachment, at: 0, effectiveRange: nil) as! NSTextAttachment
    precondition(decodedIcon.attachmentCell?.cellSize() == NSSize(width: 64, height: 64),
        "Installer icon must retain its 64 pt display size")
    // productbuild accepts RTF files; the final package receives the RTFD bundle below.
    try Data(#"{\rtf1\ansi Soundy}"#.utf8).write(to: destination.appendingPathComponent(name + ".rtf"))
}

try page("welcome", title: "Soundy", subtitle: "Звук каждого нажатия.",
    detail: "40 профилей клавиатуры · 15 звуков мыши\nSoundy будет установлен в «Программы».\nПосле установки откройте приложение\nи выберите звук в строке меню.")
try page("conclusion", title: "Готово", subtitle: "Soundy установлен.",
    detail: "Откройте Soundy в «Программах».\nВ строке меню нажмите «Разрешить мониторинг ввода…»\nи включите доступ в настройках macOS.\nВыберите звук — и печатайте с удовольствием.")
