// Renders packaging/landingcraft.svg as AppIcon.icns, placed on the macOS app
// icon grid: an 824-point body centred on a 1024-point canvas, with the
// standard drop shadow. The .icns is committed; rerun this if the SVG changes:
//
//   swift packaging/macos/make-icon.swift packaging/landingcraft.svg packaging/macos/AppIcon.icns

import AppKit

let args = CommandLine.arguments
guard args.count == 3, let svg = NSImage(contentsOfFile: args[1]) else {
    FileHandle.standardError.write("usage: make-icon.swift <icon.svg> <out.icns>\n".data(using: .utf8)!)
    exit(1)
}

func render(_ px: Int) -> Data {
    let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: px, pixelsHigh: px, bitsPerSample: 8, samplesPerPixel: 4,
        hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    let k = CGFloat(px) / 1024
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    NSGraphicsContext.current!.imageInterpolation = .high
    let shadow = NSShadow()
    shadow.shadowOffset = NSSize(width: 0, height: -10 * k)
    shadow.shadowBlurRadius = 10 * k
    shadow.shadowColor = NSColor.black.withAlphaComponent(0.3)
    shadow.set()
    svg.draw(in: NSRect(x: 100 * k, y: 100 * k, width: 824 * k, height: 824 * k))
    NSGraphicsContext.restoreGraphicsState()
    return rep.representation(using: .png, properties: [:])!
}

let iconset = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("AppIcon-\(getpid()).iconset")
try? FileManager.default.removeItem(at: iconset)
try! FileManager.default.createDirectory(at: iconset, withIntermediateDirectories: true)
for size in [16, 32, 128, 256, 512] {
    try! render(size).write(to: iconset.appendingPathComponent("icon_\(size)x\(size).png"))
    try! render(size * 2).write(to: iconset.appendingPathComponent("icon_\(size)x\(size)@2x.png"))
}

let iconutil = Process()
iconutil.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
iconutil.arguments = ["-c", "icns", iconset.path, "-o", args[2]]
try! iconutil.run()
iconutil.waitUntilExit()
try? FileManager.default.removeItem(at: iconset)
exit(iconutil.terminationStatus)
