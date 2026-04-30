import { network } from "hardhat";
const { ethers } = await network.connect();

async function main() {
  const [signer] = await ethers.getSigners();
  const SEOA = await ethers.getContractFactory("sEOA");

  const FakeDdexSequencer = await ethers.getContractFactory(
    "FakeDdexSequencer",
  );
  const fakeDdexSequencer = await FakeDdexSequencer.deploy();
  const seoaImplementation = await SEOA.deploy();

  const auth = await signer.authorize({
    address: await seoaImplementation.getAddress(),
    nonce: (await signer.getNonce()) + 1,
  });

  const tx = await signer.sendTransaction({
    to: signer.address,
    authorizationList: [auth],
    data: SEOA.interface.encodeFunctionData("setDdexSequencerAddress", [
      await fakeDdexSequencer.getAddress(),
    ]),
  });
  await tx.wait();
  console.log(tx.hash);
}

main();
