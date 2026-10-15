// Dioptra landing page: theme toggle, the engraved scale under the hero, and download
// links read from the latest GitHub release. Works without JavaScript too: every download
// button then points at the releases page.
(function () {
  'use strict';

  var REPO = 'ZahakJ/dioptra';
  var RELEASES = 'https://github.com/' + REPO + '/releases/latest';
  var API = 'https://api.github.com/repos/' + REPO + '/releases/latest';

  // ---- Theme ----------------------------------------------------------------

  var root = document.documentElement;
  var toggle = document.getElementById('theme-toggle');

  function systemDark() {
    return !!(window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches);
  }
  function currentTheme() {
    var t = root.getAttribute('data-theme');
    if (t === 'light' || t === 'dark') return t;
    return systemDark() ? 'dark' : 'light';
  }
  function labelToggle() {
    if (!toggle) return;
    var next = currentTheme() === 'dark' ? 'light' : 'dark';
    toggle.setAttribute('aria-label', 'Switch to ' + next + ' theme');
  }
  if (toggle) {
    toggle.addEventListener('click', function () {
      var next = currentTheme() === 'dark' ? 'light' : 'dark';
      root.setAttribute('data-theme', next);
      try { localStorage.setItem('dioptra-theme', next); } catch (e) { /* not persisted */ }
      labelToggle();
    });
    labelToggle();
    if (window.matchMedia) {
      var mq = window.matchMedia('(prefers-color-scheme: dark)');
      if (mq.addEventListener) mq.addEventListener('change', labelToggle);
    }
  }

  // ---- The engraved scale ----------------------------------------------------

  var ticks = document.getElementById('ticks');
  if (ticks) {
    var ns = 'http://www.w3.org/2000/svg';
    for (var i = 0; i <= 100; i++) {
      var x = i * 10 + 0.5;
      var major = i % 10 === 0;
      var mid = i % 5 === 0;
      var line = document.createElementNS(ns, 'line');
      line.setAttribute('x1', x);
      line.setAttribute('x2', x);
      line.setAttribute('y1', major ? 6 : mid ? 13 : 17);
      line.setAttribute('y2', 23.5);
      if (major) line.setAttribute('class', 'major');
      if (i === 62) { line.setAttribute('class', 'mark'); line.setAttribute('y1', 2); }
      ticks.appendChild(line);
    }
  }

  // ---- Downloads -------------------------------------------------------------

  var status = document.getElementById('release-status');

  // Which asset each button wants, by file name.
  var PICK = {
    'appimage': function (n) { return /\.AppImage$/i.test(n); },
    'deb': function (n) { return /\.deb$/i.test(n); },
    'rpm': function (n) { return /\.rpm$/i.test(n); },
    'msi': function (n) { return /\.msi$/i.test(n); },
    'exe': function (n) { return /setup\.exe$/i.test(n) || (/\.exe$/i.test(n) && !/\.msi/i.test(n)); },
    'dmg-arm': function (n) { return /(aarch64|arm64|universal)\.dmg$/i.test(n); },
    'dmg-x64': function (n) { return /(x64|x86_64|universal)\.dmg$/i.test(n); }
  };

  function size(bytes) {
    if (!bytes) return '';
    var mb = bytes / (1024 * 1024);
    return (mb >= 10 ? mb.toFixed(0) : mb.toFixed(1)) + ' MB';
  }

  function setStatus(html) {
    if (status) status.innerHTML = html;
  }

  function apply(release) {
    var assets = (release && release.assets) || [];
    var version = String(release.tag_name || '').replace(/^v/, '');
    var found = 0;
    var links = document.querySelectorAll('a.dl[data-asset]');
    for (var i = 0; i < links.length; i++) {
      var a = links[i];
      var want = PICK[a.getAttribute('data-asset')];
      var asset = null;
      for (var j = 0; j < assets.length && want; j++) {
        if (want(assets[j].name) && !/\.(sig|sha256|asc)$/i.test(assets[j].name)) { asset = assets[j]; break; }
      }
      if (!asset) continue;
      found++;
      a.href = asset.browser_download_url;
      a.title = asset.name;
      var meta = a.querySelector('.dl-meta');
      if (meta) meta.textContent = size(asset.size);
    }
    if (found === 0) {
      setStatus('No files are attached to the latest release yet; the <a href="' + RELEASES + '">releases page</a> lists what there is.');
      return;
    }
    var date = release.published_at ? new Date(release.published_at) : null;
    var when = date && !isNaN(date) ? ', published ' + date.toISOString().slice(0, 10) : '';
    setStatus('Version <strong>' + escapeHtml(version) + '</strong>' + when +
      '. Release notes and checksums are on the <a href="' + escapeAttr(release.html_url || RELEASES) + '">releases page</a>.');
  }

  function escapeHtml(s) {
    return String(s).replace(/[&<>"']/g, function (c) {
      return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c];
    });
  }
  function escapeAttr(s) {
    var v = String(s);
    return /^https:\/\/github\.com\//.test(v) ? escapeHtml(v) : RELEASES;
  }

  function fallback() {
    setStatus('GitHub could not be reached just now. Every file is on the <a href="' + RELEASES + '">releases page</a>.');
  }

  function noRelease() {
    setStatus('No release is published yet. When it is, every file will be on the <a href="' + RELEASES + '">releases page</a>.');
  }

  function markMine() {
    var p = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || '';
    var ua = navigator.userAgent || '';
    var os = /Win/i.test(p) ? 'windows' : /Mac/i.test(p) && !/iPhone|iPad/i.test(ua) ? 'mac' : /Linux|X11/i.test(p) && !/Android/i.test(ua) ? 'linux' : '';
    if (!os) return;
    var card = document.querySelector('.platform[data-os="' + os + '"]');
    if (!card) return;
    card.classList.add('is-mine');
    var h = card.querySelector('h3');
    if (h) {
      var tag = document.createElement('span');
      tag.className = 'yours';
      tag.textContent = 'This computer';
      h.appendChild(tag);
    }
  }
  markMine();

  if (window.fetch && document.querySelector('a.dl[data-asset]')) {
    var done = false;
    var timer = setTimeout(function () { if (!done) { done = true; fallback(); } }, 8000);
    fetch(API, { headers: { 'Accept': 'application/vnd.github+json' } })
      .then(function (r) {
        if (r.status === 404) { done = true; clearTimeout(timer); noRelease(); return null; }
        if (!r.ok) throw new Error('HTTP ' + r.status);
        return r.json();
      })
      .then(function (rel) { if (done || !rel) return; done = true; clearTimeout(timer); apply(rel); })
      .catch(function () { if (done) return; done = true; clearTimeout(timer); fallback(); });
  }
})();
