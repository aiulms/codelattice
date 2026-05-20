#!/usr/bin/env node

import { getAppInfo } from '../src/index.js';

const info = getAppInfo();
console.log(JSON.stringify(info, null, 2));
