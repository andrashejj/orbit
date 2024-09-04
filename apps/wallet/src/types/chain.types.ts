export enum BlockchainType {
  InternetComputer = 'icp',
  Bitcoin = 'btc',
  Ethereum = 'eth',
  EthereumSepolia = 'eth_sepolia',
}

export enum BlockchainStandard {
  Native = 'native',
  ERC20 = 'erc20',
}

export enum TokenSymbol {
  ICP = 'ICP',
  ETH = 'ETH',
  ETH_SEPOLIA = 'ETH Sepolia',
}

export interface FetchTransfersInput {
  fromDt?: Date;
  toDt?: Date;
}

export interface AccountIncomingTransfer {
  from: string;
  to: string;
  amount: bigint;
  fee: bigint;
  created_at?: Date;
  confirmations?: number;
}

export interface FetchTransfersResponse {
  transfers: AccountIncomingTransfer[];
}

export interface ChainApi {
  fetchTransfers(input: FetchTransfersInput): Promise<AccountIncomingTransfer[]>;

  isValidAddress(address: string): boolean;
}
