import './sui_usdc_dubhe_wallet.js';
import QRCode from 'qrcode';

// Only pairing URIs enter the QR renderer; no game token or checkout capability.
window.Mir2DubheQr = Object.freeze({
  toDataURL(uri) {
    if (typeof uri !== 'string' || uri.length > 4096 || !/^wc:[^\s]+@2\?/.test(uri)) throw Error('二维码无效');
    return QRCode.toDataURL(uri, { width: 320, margin: 3, errorCorrectionLevel: 'M' });
  },
});
