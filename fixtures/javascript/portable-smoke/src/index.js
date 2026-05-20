import { add, multiply } from './math.js';
import logger from './logger.cjs';
import { default as utils } from './utils.js';

const result = add(1, 2);
const product = multiply(3, 4);

export const appName = 'codelattice-js-portable-smoke';

export function getAppInfo() {
    return {
        name: appName,
        version: '1.0.0',
        result,
        product
    };
}

export default function createApp() {
    return {
        run: () => {
            logger.info('App running');
            return getAppInfo();
        }
    };
}
