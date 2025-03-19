// Done to appease issue in generated code:
//  error TS4111: Property 'NODE_ENV' comes from an index signature, so it must be accessed with ['NODE_ENV'].

declare namespace NodeJS {
  interface ProcessEnv {
    NODE_ENV: string;
  }
}
