// Checkout settings for the pricing page. Leave a price id empty and its button sends an
// email instead. Paddle price ids look like "pri_01h..."; the client-side token looks like
// "test_..." in the sandbox and "live_..." in production.
window.DIOPTRA_CHECKOUT = {
  provider: 'paddle',
  environment: 'sandbox', // 'sandbox' | 'production'
  clientToken: '',
  prices: {
    academic: '',
    corporate: '',
    founding: '',
    dept_site: '',
    inst_site: ''
  }
};
