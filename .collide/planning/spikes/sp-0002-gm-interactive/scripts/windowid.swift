import CoreGraphics
import Foundation
// usage: windowid <pid>  -> prints "<windowid> <x> <y> <w> <h> <title>" for each on-screen window of the pid
let pid = Int32(CommandLine.arguments[1])!
let list = CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as! [[String: Any]]
for w in list where (w[kCGWindowOwnerPID as String] as? Int32) == pid {
  let b = w[kCGWindowBounds as String] as! [String: Any]
  print(w[kCGWindowNumber as String]!, b["X"]!, b["Y"]!, b["Width"]!, b["Height"]!, w[kCGWindowLayer as String]!, (w[kCGWindowName as String] as? String) ?? "")
}
