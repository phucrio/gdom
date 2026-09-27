interface DriveAuthQaState {
  listCalls: number;
  reauthenticationCalls: number;
  authorizationRequired: boolean;
  cancelFirstReauthentication: boolean;
  commands: string[];
  completeReauthentication: () => void;
}

declare global {
  interface Window {
    driveAuthQa: DriveAuthQaState;
  }
}

export {};
