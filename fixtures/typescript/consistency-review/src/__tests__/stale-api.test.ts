// Stale test — references OldClient and removedHelper which no longer exist
// This file should be flagged as a stale test candidate

// OldClient was removed in v0.3
function testOldClient() {
  // OldClient is no longer defined anywhere
  const client = OldClient({});
}

// removedHelper was deleted
function testRemovedHelper() {
  // removedHelper is not defined
}
