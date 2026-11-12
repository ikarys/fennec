// Adds page eyebrows, callout variants, and an on-page table of contents.
function eyebrow() {
  const active = document.querySelector('.sidebar .chapter a.active');
  if (!active) return;
  let li = active.closest('li');
  while (li) {
    li = li.previousElementSibling;
    if (!li) return;
    if (li.classList.contains('part-title')) {
      const h1 = document.querySelector('.content main h1');
      if (h1) { const d = document.createElement('div'); d.className = 'page-eyebrow'; d.textContent = li.textContent.trim(); h1.before(d); }
      return;
    }
  }
}
function callouts() {
  document.querySelectorAll('.content blockquote').forEach((bq) => {
    const p = bq.querySelector('p');
    if (!p) return;
    const strong = p.querySelector('strong');
    if (!strong || p.firstElementChild !== strong) return;
    const label = strong.textContent.trim().replace(/\s*:\s*$/, '').trim().toLowerCase();
    if (label === 'tip') bq.classList.add('callout-tip');
    if (label === 'warning' || label === 'important') bq.classList.add('callout-warn');
  });
}
function pageToc() {
  const headings = Array.from(document.querySelectorAll('.content main h2')).filter((h) => h.id);
  if (headings.length < 2) return;
  const nav = document.createElement('nav');
  nav.className = 'page-toc';
  const title = document.createElement('div');
  title.className = 'page-toc-title';
  title.textContent = 'On this page';
  nav.appendChild(title);
  const links = headings.map((h) => {
    const a = document.createElement('a');
    a.href = '#' + h.id;
    a.textContent = h.textContent;
    nav.appendChild(a);
    return a;
  });
  document.body.appendChild(nav);
  const observer = new IntersectionObserver((entries) => {
    entries.forEach((entry) => {
      if (!entry.isIntersecting) return;
      const id = entry.target.id;
      links.forEach((link) => link.classList.toggle('current', link.getAttribute('href') === '#' + id));
    });
  }, { rootMargin: '0px 0px -70% 0px' });
  headings.forEach((h) => observer.observe(h));
}
eyebrow();
callouts();
pageToc();
