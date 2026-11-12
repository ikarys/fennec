// Click a screenshot to see it full size; click again or press Escape to close.
document.addEventListener('click', (event) => {
  const open = document.querySelector('.zoom-overlay');
  if (open) {
    open.remove();
    return;
  }
  const img = event.target.closest('.content img');
  if (!img) return;
  const overlay = document.createElement('div');
  overlay.className = 'zoom-overlay';
  overlay.append(img.cloneNode());
  document.body.append(overlay);
});
document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape') document.querySelector('.zoom-overlay')?.remove();
});
