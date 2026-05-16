// Test file — should be excluded when includeTests=false
import { oldHelper } from "../legacy";

function testHelper(): void {
  console.log("test setup");
}

function testOldHelper(): void {
  const result = oldHelper(5);
  if (result !== 10) {
    throw new Error("expected 10");
  }
}
