// Note that a dynamic `import` statement here is required due to
// webpack/webpack#6615, but in theory `import { greet } from './pkg';`

// TODO: IMPORTANT NOTICE! THIS IS WORKS ONLY IF BUILT WITH npm run build:node
const dpp_module = require('./pkg');
const assert = require('assert');

const { IdentityFacade } = dpp_module;

const identityFacade = new IdentityFacade();

const validationResult = identityFacade.validate({
            "protocolVersion":1,
            "id": [198, 23, 40, 120, 58, 93, 0, 165, 27, 49, 4, 117, 107, 204,  67, 46, 164, 216, 230, 135, 201, 92, 31, 155, 62, 131, 211, 177, 139, 175, 163, 237],
            "publicKeys": [
                {"id":0,"type":0,"purpose":0,"securityLevel":0,"data":"AuryIuMtRrl/VviQuyLD1l4nmxi9ogPzC9LT7tdpo0di","readOnly":false},
                {"id":1,"type":0,"purpose":1,"securityLevel":3,"data":"A8AK95PYMVX5VQKzOhcVQRCUbc9pyg3RiL7jttEMDU+L","readOnly":false}
            ],
            "balance":10,
            "revision":0
    }
);

assert(validationResult.isValid());

console.log('is valid first?', validationResult.isValid())
console.log(validationResult.errorsText());

const validationResult2 = identityFacade.validate({
        "protocolVersion": 1,
        "id": [198, 23, 40, 120, 58, 93, 0, 165, 27, 49, 4, 117, 107, 204,  67, 46, 164, 216, 230, 135, 201, 92, 31, 155, 62, 131, 211, 177, 139, 175, 163, 237],
        "publicKeys": [
            {"id":0,"type":0,"purpose":0,"securityLevel":0,"data":"AuryIuMtRrl/VviQuyLD1l4nmxi9ogPzC9LT7tdpo0di","readOnly":false},
            {"id":1,"type":0,"purpose":1,"securityLevel":3,"data":"A8AK95PYMVX5VQKzOhcVQRCUbc9pyg3RiL7jttEMDU+L","readOnly":false}
        ],
        "balance": "this is not a correct balance",
        "revision":0
    }
);

assert(validationResult2.isValid() === false);

console.log('is valid second?', validationResult2.isValid())
console.log(validationResult2.errorsText());

// rust
//     .then(dpp => {
//         const { Identifier, Identity } = dpp;
//         const identifier = Identifier.fromString("EDCuAy8AXqAh56eFRkKRKb79SC35csP3W9VPe1UMaz87")
//         console.log(identifier.toString());
//         const buf = identifier.toBuffer();
//         console.log(identifier.toBuffer());
//         console.log(Array.from(identifier.toBuffer()).map(u8 => u8.toString(16)).join(''))
//         const id2 = new Identifier(identifier.toBuffer());
//         console.log('id2', id2.toString());
//         const id3 = Identifier.from("EDCuAy8AXqAh56eFRkKRKb79SC35csP3W9VPe1UMaz87")
//         console.log('id3', id3.toString());
//         console.log('buf:', buf);
//         const id4 = Identifier.from(buf);
//         console.log('id4', id4.toString());
//
//         let i = Identity.new();
//         console.log("the originl object", i);
//         console.log("the identity", i.toString());
//         console.log("the identity", i.toObject());
//         console.log("the public keys", i.getPublicKeys());
//     })
//     .catch(console.error);
