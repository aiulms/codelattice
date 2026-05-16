// Entry point — imports and uses live functions
import { liveFunction } from "./live";
import { publicUtility } from "./public-api";

console.log(liveFunction("hello"));
console.log(publicUtility());
