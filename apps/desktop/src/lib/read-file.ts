/**
 * Read a picked file as text.
 *
 * `Blob.text()` is the obvious call and is what this uses when it exists. It
 * needs Safari 14+, though, and the desktop shell runs on WKWebView — so an
 * older macOS would throw "file.text is not a function" at the one moment the
 * user has just chosen a file. `FileReader` has been there since forever.
 *
 * jsdom is missing `Blob.text()` too, which is how this came up.
 */
export function readTextFile(file: File): Promise<string> {
  if (typeof file.text === "function") return file.text();
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () =>
      reject(reader.error ?? new Error(`could not read ${file.name}`));
    reader.onload = () => resolve(String(reader.result ?? ""));
    reader.readAsText(file);
  });
}
