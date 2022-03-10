// Note that a dynamic `import` statement here is required due to
// webpack/webpack#6615, but in theory `import { greet } from './pkg';`


// will work here one day as well!
const rust = import('./pkg');

rust
    .then(dpp => {
        const { Identifier, Identity } = dpp;
        const identifier = Identifier.fromString("EDCuAy8AXqAh56eFRkKRKb79SC35csP3W9VPe1UMaz87")
        console.log(identifier.toString());
        const buf = identifier.toBuffer();
        console.log(identifier.toBuffer());
        console.log(Array.from(identifier.toBuffer()).map(u8 => u8.toString(16)).join(''))
        const id2 = new Identifier(identifier.toBuffer());
        console.log('id2', id2.toString());
        const id3 = Identifier.from("EDCuAy8AXqAh56eFRkKRKb79SC35csP3W9VPe1UMaz87")
        console.log('id3', id3.toString());
        console.log('buf:', buf);
        const id4 = Identifier.from(buf);
        console.log('id4', id4.toString());

        let i = Identity.new();
        console.log("the originl object", i);
        console.log("the identity", i.toString());
        console.log("the identity", i.toObject());
        console.log("the public keys", i.getPublicKeys());

    })
    .catch(console.error);
