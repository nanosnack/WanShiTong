// The part of Alpine's API our scripts use. Alpine itself loads later,
// deferred, so components are registered on `alpine:init`.
//
// Each component types its own `this`, magics included, with `ThisType`.
declare const Alpine: {
  data(name: string, component: (...args: never[]) => object): void;
};
