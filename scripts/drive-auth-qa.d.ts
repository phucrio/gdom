interface DriveAuthQaState {
  listCalls: number;
  reauthenticationCalls: number;
  accountRefreshCalls: number;
  authorizationRequired: boolean;
  cancelFirstReauthentication: boolean;
  commands: string[];
}

declare global {
  interface Window {
    driveAuthQa: DriveAuthQaState;
  }
}

export {};
