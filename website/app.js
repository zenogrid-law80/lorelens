document.querySelector('#copy').addEventListener('click', async () => {
  const status = document.querySelector('#copy-status');
  try { await navigator.clipboard.writeText(document.querySelector('#install-command').textContent); status.textContent = 'Commands copied. Update the repository path before running them.'; }
  catch { status.textContent = 'Automatic copying is unavailable. Select and copy the commands above.'; }
});
