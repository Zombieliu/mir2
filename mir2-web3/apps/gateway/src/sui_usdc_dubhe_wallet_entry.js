import './sui_usdc_dubhe_wallet.js';
import QRCode from 'qrcode';
import { createDirectTransferCard, validateDirectPaymentRequestUrl } from './sui_usdc_direct_card.js';

window.Mir2DirectTransferCard = Object.freeze({ create: createDirectTransferCard });

// Neither QR path receives a game token or checkout capability.
window.Mir2DubheQr = Object.freeze({
  toDataURL(uri) {
    if (typeof uri !== 'string' || uri.length > 4096 || !/^wc:[^\s]+@2\?/.test(uri)) throw Error('二维码无效');
    return QRCode.toDataURL(uri, { width: 320, margin: 3, errorCorrectionLevel: 'M' });
  },
  toPaymentDataURL(url) {
    if (!validateDirectPaymentRequestUrl(url)) throw Error('付款二维码无效');
    return QRCode.toDataURL(url, { width: 320, margin: 3, errorCorrectionLevel: 'M' });
  },
});
