/* Light / dark switch, shared by every Pink Bourbon Estate site.
   Without this script the page simply follows your device's setting.
   With it, the button lets you choose, and your choice is remembered in this browser only
   (localStorage: nothing is sent anywhere, no cookie is set). */
(() => {
  const root = document.documentElement;
  const KEY = 'theme';
  // apply a saved choice straight away, before the page is drawn, so there is no flash
  try { const saved = localStorage.getItem(KEY); if (saved === 'light' || saved === 'dark') root.dataset.theme = saved; } catch (e) { /* storage blocked: follow the device */ }

  const isDark = () => root.dataset.theme === 'dark' || (!root.dataset.theme && matchMedia('(prefers-color-scheme: dark)').matches);

  document.addEventListener('DOMContentLoaded', () => {
    const header = document.querySelector('header.top');
    if (!header) return;
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'themebtn';
    const paint = () => { btn.textContent = isDark() ? 'Light' : 'Dark'; btn.setAttribute('aria-label', isDark() ? 'Switch to light mode' : 'Switch to dark mode'); };
    btn.addEventListener('click', () => {
      const next = isDark() ? 'light' : 'dark';
      root.dataset.theme = next;
      try { localStorage.setItem(KEY, next); } catch (e) { /* not saved, still works for this visit */ }
      paint();
    });
    paint();
    header.append(btn);
  });
})();
