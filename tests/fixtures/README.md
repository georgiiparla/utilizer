`background.log` contains a short excerpt adapted from the console log supplied
in this conversation. Identifying paths and player names have been replaced;
timestamps, unrelated output, Unicode cases (covered in parser tests), and
multiline structure are the relevant features. It contains no capture protocol.

`synthetic_capture.log` is deliberately synthetic. The existing Ruby generator
establishes the marker strings and `getpos` request, but the supplied real log
does not show their output. These fixtures test Utilizer's implementation, not
CS2's actual echo prefix, `getpos` formatting, or flushing behavior.
