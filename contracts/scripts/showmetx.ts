import { network } from "hardhat";
const { ethers } = await network.connect();

async function main() {
  const receipt = await ethers.provider.getTransactionReceipt(
    "0x4a9b410f6ea10bdc1dd874a246f591ac4ad1880b8598f9cf97846d49c5148af2",
  );
  console.log({ receipt });
}

main();
