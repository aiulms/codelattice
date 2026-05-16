// Test file — should be excluded when includeTests=false
import { validateInput, transform } from "../domain/validator";
import { handleRequest } from "../service/handler";

function testValidation(): void {
  if (!validateInput("hello")) throw new Error("fail");
}

function testTransform(): void {
  const result = transform("test", { prefix: "X_" });
  if (result !== "X_TEST") throw new Error("fail");
}

function testHandler(): void {
  handleRequest("input");
}
