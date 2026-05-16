// Test file: test helper should only appear when includeTests=true
import { legacyProcess } from '../legacy';

export function testHelper() {
  return legacyProcess();
}

export function runTest() {
  testHelper();
}
