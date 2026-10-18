// Loaded only by the isolated preview. Reconnect across failed builds/restarts.
(function () {
  let polling = false;
  setInterval(async function () {
    if (polling) return;
    polling = true;
    try {
      const reply = await fetch('/__dev/revision', {
        cache: 'no-store', signal: AbortSignal.timeout(1500)
      });
      if (reply.ok && (await reply.text()) !== utilizerPreviewVersion) location.reload();
    } catch (_) { /* The development launcher is rebuilding or stopped. */ }
    finally { polling = false; }
  }, 500);
})();
