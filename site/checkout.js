// Buy buttons on the pricing page. Reads window.DIOPTRA_CHECKOUT (checkout-config.js).
// A button whose plan has a price id opens Paddle checkout; otherwise it is an email to
// CONTACT_EMAIL. Paddle.js is loaded only when someone presses a configured button, so the
// page itself sets no cookies and loads nothing from a third party.
(function () {
  'use strict';

  // Replace with the real address before launch.
  var CONTACT_EMAIL = 'CONTACT_EMAIL@example.invalid';
  var PADDLE_JS = 'https://cdn.paddle.com/paddle/v2/paddle.js';

  var NAMES = {
    academic: 'Academic',
    corporate: 'Corporate',
    founding: 'Founding member',
    dept_site: 'Department site',
    inst_site: 'Institution site',
    corp_site: 'Corporate site'
  };

  var cfg = window.DIOPTRA_CHECKOUT || {};
  var prices = cfg.prices || {};
  var paddleReady = null;

  function mailto(plan) {
    var name = NAMES[plan] || 'Lab';
    var subject = plan === 'corp_site' ? 'Dioptra Lab: corporate site quote' : 'Dioptra Lab: ' + name + ' license';
    var body = plan === 'corp_site' || plan === 'dept_site' || plan === 'inst_site'
      ? 'Organisation:\nNumber of people:\nInvoice or purchase order:\n'
      : 'Name:\nOrganisation:\n';
    return 'mailto:' + CONTACT_EMAIL + '?subject=' + encodeURIComponent(subject) +
      '&body=' + encodeURIComponent(body);
  }

  function configured(plan) {
    return cfg.provider === 'paddle' && !!cfg.clientToken && !!prices[plan];
  }

  function loadPaddle() {
    if (paddleReady) return paddleReady;
    paddleReady = new Promise(function (resolve, reject) {
      if (window.Paddle) { resolve(window.Paddle); return; }
      var s = document.createElement('script');
      s.src = PADDLE_JS;
      s.async = true;
      s.onload = function () { window.Paddle ? resolve(window.Paddle) : reject(new Error('Paddle missing')); };
      s.onerror = function () { reject(new Error('Paddle.js did not load')); };
      document.head.appendChild(s);
    }).then(function (Paddle) {
      if (cfg.environment === 'sandbox') Paddle.Environment.set('sandbox');
      Paddle.Initialize({ token: cfg.clientToken });
      return Paddle;
    });
    paddleReady.catch(function () { paddleReady = null; });
    return paddleReady;
  }

  // Contact links and addresses written into the page text.
  var contacts = document.querySelectorAll('[data-contact]');
  for (var i = 0; i < contacts.length; i++) {
    var el = contacts[i];
    if (el.tagName === 'A') el.href = mailto(el.getAttribute('data-contact') || 'corp_site');
    if (el.hasAttribute('data-contact-text')) el.textContent = CONTACT_EMAIL;
  }

  var buttons = document.querySelectorAll('[data-plan]');
  for (var j = 0; j < buttons.length; j++) {
    (function (b) {
      var plan = b.getAttribute('data-plan');
      if (!configured(plan)) {
        b.href = mailto(plan);
        return;
      }
      b.href = '#';
      b.addEventListener('click', function (e) {
        e.preventDefault();
        loadPaddle().then(function (Paddle) {
          Paddle.Checkout.open({ items: [{ priceId: prices[plan], quantity: 1 }] });
        }).catch(function () {
          window.location.href = mailto(plan);
        });
      });
    })(buttons[j]);
  }
})();
