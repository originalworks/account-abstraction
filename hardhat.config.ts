import type { HardhatUserConfig } from "hardhat/config";

import hardhatEthers from '@nomicfoundation/hardhat-ethers'
import hardhatEthersChaiMatchers from '@nomicfoundation/hardhat-ethers-chai-matchers' 
import hardhatKeystore from '@nomicfoundation/hardhat-keystore' 
import hardhatMocha from '@nomicfoundation/hardhat-mocha' 
import hardhatNetworkHelpers from '@nomicfoundation/hardhat-network-helpers' 
import hardhatTypechain from '@nomicfoundation/hardhat-typechain' 
import hardhatVerify from '@nomicfoundation/hardhat-verify'

import { configVariable } from "hardhat/config";

const config: HardhatUserConfig = {
  plugins: [hardhatEthers, hardhatEthersChaiMatchers, hardhatKeystore, hardhatMocha, hardhatNetworkHelpers, hardhatTypechain, hardhatVerify],
  solidity: {
    profiles: {
      default: {
        version: "0.8.28",
      },
      production: {
        version: "0.8.28",
        settings: {
          optimizer: {
            enabled: true,
            runs: 200,
          },
        },
      },
    },
  },
  typechain: {
    outDir: 'typechain',
    
  },
  networks: {
    localhost: {
      type: "http",
      chainType: 'l1',
      url: 'http://127.0.0.1:8545',
      accounts: {mnemonic: "test test test test test test test test test test test junk"}
    },
    hardhatMainnet: {
      type: "edr-simulated",
      chainType: "l1",
    },
    hardhatOp: {
      type: "edr-simulated",
      chainType: "op",
    },
    sepolia: {
      type: "http",
      chainType: "l1",
      url: 'https://eth-sepolia.g.alchemy.com/v2/PlbpBvp7JeNHwWFG0TY1X',
      accounts: [
        configVariable("WALLET_ONE_PK"),
        configVariable("WALLET_TWO_PK")
      ]
    },
  },
};

export default config;
