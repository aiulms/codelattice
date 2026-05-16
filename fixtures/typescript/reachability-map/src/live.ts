// Live handler: reachable from index → app → live
export function liveHandler() {
  console.log('Live handler active');
}

export function liveHelper() {
  return 'helper';
}
