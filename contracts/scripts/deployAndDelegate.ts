import { network } from "hardhat";
const { ethers } = await network.connect();

async function main() {
  const [signer] = await ethers.getSigners();
  const SEOA = await ethers.getContractFactory("sEOA");

  const seoaImplementation = await SEOA.deploy();

  const auth = await signer.authorize({
    address: await seoaImplementation.getAddress(),
    nonce: (await signer.getNonce()) + 1,
  });

  const tx = await signer.sendTransaction({
    to: signer.address,
    authorizationList: [auth],
    data: SEOA.interface.encodeFunctionData("usedSalts", [
      "0x0000000000000000000000000000000000000000000000000000000000000000",
    ]),
  });
  await tx.wait();
  console.log(tx.hash);
}

main();
