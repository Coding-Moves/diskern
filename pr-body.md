Automated dependency refresh.

## What ran

- `cargo audit fix` — raises version requirements that an advisory needs (experimental).
- `cargo update` — refreshes `Cargo.lock` within the existing semver ranges.
- `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` — all green before this PR opened.

## Changes

```diff
diff --git a/Cargo.lock b/Cargo.lock
index aca90c6..9b202b1 100644
--- a/Cargo.lock
+++ b/Cargo.lock
@@ -10,9 +10,9 @@ checksum = "320119579fcad9c21884f5c4861d16174d0e06250625266f50fe6898340abefa"
 
 [[package]]
 name = "aho-corasick"
-version = "1.1.4"
+version = "1.1.5"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "ddd31a130427c27518df266943a5308ed92d4b226cc639f5a8f1002816174301"
+checksum = "c982642fa9e8606056828ee9a8505737230110bb1099153c79efe865c59d12ba"
 dependencies = [
  "memchr",
 ]
@@ -34,9 +34,9 @@ dependencies = [
 
 [[package]]
 name = "android_system_properties"
-version = "0.1.5"
+version = "0.1.6"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "819e7219dbd41043ac279b19830f2efc897156490d7fd6ea916720117ee66311"
+checksum = "ae221649c9976a6f6c56ae1facf410f3ddb33cc661c4b7b61020a912d4237fbc"
 dependencies = [
  "libc",
 ]
@@ -199,7 +199,7 @@ dependencies = [
  "cc",
  "cfg-if",
  "constant_time_eq",
- "cpufeatures 0.3.0",
+ "cpufeatures 0.3.1",
  "memmap2",
  "rayon-core",
 ]
@@ -260,9 +260,9 @@ checksum = "72f5acc6cb2ba439de613abc23857ec3d78374d8ed5ac84e9d11336e87da8649"
 
 [[package]]
 name = "bytemuck"
-version = "1.25.1"
+version = "1.25.2"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "d6aedf8ae72766347502cf3cb4f41cf5e9cc37d28bee90f1fdaaae15f9cf9424"
+checksum = "95832e849adfb21180ccb6826a99da14e5d266ae5c2e668e1602cf234f153797"
 
 [[package]]
 name = "byteorder"
@@ -306,9 +306,9 @@ dependencies = [
 
 [[package]]
 name = "camino"
-version = "1.2.4"
+version = "1.2.5"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "5f2d30e4173c4026932d51d31d6b0613b1fd3014bf3f9f8943d4ba139c437ba0"
+checksum = "bb1307f12aa967b5a58416e87b3653360e0fd614a016b6e970db08fecbb1b80d"
 dependencies = [
  "serde_core",
 ]
@@ -348,9 +348,9 @@ dependencies = [
 
 [[package]]
 name = "cc"
-version = "1.3.0"
+version = "1.4.5"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "c89588d05638b5b4594a3348a2d6c20277e43a7f5c5202b05cc56888475a47b8"
+checksum = "005ec2760ca554fae18df7a11195552ec576cd665632a881bc011d5bb2fd4d80"
 dependencies = [
  "find-msvc-tools",
  "shlex",
@@ -403,9 +403,9 @@ dependencies = [
 
 [[package]]
 name = "clap"
-version = "4.6.3"
+version = "4.6.6"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "0fb99565819980999fb7b4a1796046a5c949e6d4ff132cf5fadf5a641e20d776"
+checksum = "473c7e07f409a8d772161724aa8db6a765a2532a70f9667eeb7b49d3d02fbdca"
 dependencies = [
  "clap_builder",
  "clap_derive",
@@ -413,9 +413,9 @@ dependencies = [
 
 [[package]]
 name = "clap_builder"
-version = "4.6.2"
+version = "4.6.6"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "f09628afdcc538b57f3c6341e9c8e9970f18e4a481690a64974d7023bd33548b"
+checksum = "7b48fea5a88e9ae728a2dcbedbfc0e730f7d60da42e1cb049a83c9fb8b789889"
 dependencies = [
  "anstream",
  "anstyle",
@@ -425,14 +425,14 @@ dependencies = [
 
 [[package]]
 name = "clap_derive"
-version = "4.6.3"
+version = "4.6.4"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "32f2392eae7f16557a3d727ef3a12e57b2b2ca6f98566a5f4fb41ffe305df077"
+checksum = "d012d2b9d65aca7f18f4d9878a045bc17899bba951561ba5ec3c2ba1eed9a061"
 dependencies = [
  "heck 0.5.0",
  "proc-macro2",
  "quote",
- "syn 2.0.119",
+ "syn 3.0.4",
 ]
 
 [[package]]
@@ -449,9 +449,9 @@ checksum = "1d07550c9036bf2ae0c684c4297d503f838287c83c53686d05370d0e139ae570"
 
 [[package]]
 name = "combine"
-version = "4.6.7"
+version = "4.6.8"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "ba5a308b75df32fe02788e748662718f03fde005016435c444eea572398219fd"
+checksum = "cfc320937d09e6de266b31b9afb480f197d7a861be86be7cb2ea7e5d1bfffc5e"
 dependencies = [
  "bytes",
  "memchr",
@@ -465,9 +465,9 @@ checksum = "3d52eff69cd5e647efe296129160853a42795992097e8af39800e1060caeea9b"
 
 [[package]]
 name = "cookie"
-version = "0.18.1"
+version = "0.18.2"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "4ddef33a339a91ea89fb53151bd0a4689cfce27055c291dfa69945475d22c747"
+checksum = "1a373e3602691c3cdea496d2f0ee5935151e6168fe87739483c463db1b2f2f87"
 dependencies = [
  "time",
  "version_check",
@@ -534,18 +534,18 @@ dependencies = [
 
 [[package]]
 name = "cpufeatures"
-version = "0.3.0"
+version = "0.3.1"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "8b2a41393f66f16b0823bb79094d54ac5fbd34ab292ddafb9a0456ac9f87d201"
+checksum = "5ca28b0ae3115b884660db4118d803791fd6756b6e88f39c0f3f7859060d7566"
 dependencies = [
  "libc",
 ]
 
 [[package]]
 name = "crc32fast"
-version = "1.5.0"
+version = "1.5.1"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "9481c1c90cbf2ac953f07c8d4a58aa3945c425b7185c9154d67a65e4230da511"
+checksum = "8498c871161e1742aaa9d52551b2d6ebdd4c3d45a3be423e3728f33b955be550"
 dependencies = [
  "cfg-if",
 ]
@@ -714,6 +714,37 @@ dependencies = [
  "windows-sys 0.61.2",
 ]
 
+[[package]]
+name = "defmt"
+version = "1.1.1"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "e2953bfe4f93bbd20cc71198842756f77d161884c99ebbabc41d80231ded88d1"
+dependencies = [
+ "bitflags 1.3.2",
+ "defmt-macros",
+]
+
+[[package]]
+name = "defmt-macros"
+version = "1.1.1"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "bad9c72e7ca2137e0dc3813245a0d282fd6daad32fd800af018306a9169b5fe8"
+dependencies = [
+ "defmt-parser",
+ "proc-macro2",
+ "quote",
+ "syn 2.0.119",
+]
+
+[[package]]
+name = "defmt-parser"
+version = "1.0.0"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "10d60334b3b2e7c9d91ef8150abfb6fa4c1c39ebbcf4a81c2e346aad939fee3e"
+dependencies = [
+ "thiserror 2.0.20",
+]
+
 [[package]]
 name = "deranged"
 version = "0.5.8"
@@ -840,13 +871,13 @@ dependencies = [
 
 [[package]]
 name = "displaydoc"
-version = "0.2.6"
+version = "0.2.7"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "1ac70aa55017e108007fbaf5aa0f54b021c98f92ff8af59d42eda9da96e3dd4f"
+checksum = "c6232dd377dcc64799954cbd3a9bb882e9cdc1308ccd87b1c098f1fb2eaf82a8"
 dependencies = [
  "proc-macro2",
  "quote",
- "syn 2.0.119",
+ "syn 3.0.4",
 ]
 
 [[package]]
@@ -940,9 +971,9 @@ checksum = "d0881ea181b1df73ff77ffaaf9c7544ecc11e82fba9b5f27b262a3c73a332555"
 
 [[package]]
 name = "either"
-version = "1.16.0"
+version = "1.18.0"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "91622ff5e7162018101f2fea40d6ebf4a78bbe5a49736a2020649edf9693679e"
+checksum = "252afb9ae5eaa683babdc6a068b3f5726eb19e05070c731f9b2a23a7c3e8ed34"
 
 [[package]]
 name = "embed-resource"
@@ -953,7 +984,7 @@ dependencies = [
  "cc",
  "memchr",
  "rustc_version",
- "toml 1.1.3+spec-1.1.0",
+ "toml 1.1.5+spec-1.1.0",
  "vswhom",
  "winreg",
 ]
@@ -1028,9 +1059,9 @@ dependencies = [
 
 [[package]]
 name = "find-msvc-tools"
-version = "0.1.9"
+version = "0.1.12"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "5baebc0774151f905a1a2cc41989300b1e6fbb29aff0ceffa1064fdd3088d582"
+checksum = "3e0f1c7c3a72c66fd80abe965175f7523475c0489a87d3ff9d6e8c87d87a9d2d"
 
 [[package]]
 name = "fixedbitset"
@@ -1040,12 +1071,13 @@ checksum = "1d674e81391d1e1ab681a28d99df07927c6d4aa5b027d7da16ba32d1d21ecd99"
 
 [[package]]
 name = "flate2"
-version = "1.1.9"
+version = "1.1.10"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "843fba2746e448b37e26a819579957415c8cef339bf08564fe8b7ddbd959573c"
+checksum = "6e634e2e0ebac1ee034020da1ca582e17ffe4e0f5e985823721e168928136dcb"
 dependencies = [
  "crc32fast",
- "miniz_oxide",
+ "miniz_oxide 0.9.1",
+ "zlib-rs",
 ]
 
 [[package]]
@@ -1078,13 +1110,13 @@ dependencies = [
 
 [[package]]
 name = "foreign-types-macros"
-version = "0.2.3"
+version = "0.2.4"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "1a5c6c585bc94aaf2c7b51dd4c2ba22680844aba4c687be581871a6f518c5742"
+checksum = "ea5190182e6915eb873ddbc16e23b711b6eb1f9c00a0d0a3a91b5f6228475225"
 dependencies = [
  "proc-macro2",
  "quote",
- "syn 2.0.119",
+ "syn 3.0.4",
 ]
 
 [[package]]
@@ -1104,24 +1136,24 @@ dependencies = [
 
 [[package]]
 name = "futures-channel"
-version = "0.3.33"
+version = "0.3.34"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "262590f4fe6afeb0bc83be1daa64e52657fe185690a958af7f3ad0e92085c5ae"
+checksum = "b1f9e3d69d39e4862ffed03ed071a76f9a13ba1d9109d355b0f0aa6b15e393c4"
 dependencies = [
  "futures-core",
 ]
 
 [[package]]
 name = "futures-core"
-version = "0.3.33"
+version = "0.3.34"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "2cd50c473c80f6d7c3670a752354b8e569b1a7cbfdc0419ec88e5edad85e0dc7"
+checksum = "92d699e522242e69e3003b94ecc1f960f3a5e015aa7c5d7486e65ad01dd94f5e"
 
 [[package]]
 name = "futures-executor"
-version = "0.3.33"
+version = "0.3.34"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "6754879cc9f2c66f88c6e5c35344bb0bdb0708b0352b1201815667c7eabc7458"
+checksum = "031b47cf1a3c6cc8bc2fc76cd437f521619387907d469316e7c0bc278f1f5432"
 dependencies = [
  "futures-core",
  "futures-task",
@@ -1130,38 +1162,38 @@ dependencies = [
 
 [[package]]
 name = "futures-io"
-version = "0.3.33"
+version = "0.3.34"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "4577ecaa3c4f96589d473f679a71b596316f6641bc350038b962a5daf0085d7a"
+checksum = "53c0fa8157de1303bfffdaa1cc2a673bfffb60102f76b0ef4441659124373fed"
 
 [[package]]
 name = "futures-macro"
-version = "0.3.33"
+version = "0.3.34"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "2d6d3cde68c518367be28956066ddfef33813991b77a55005a69dae04bf3b10b"
+checksum = "9fb9654ba8355388abeb8dcb4fc62f511300867002afc858860463bdd9fe0c44"
 dependencies = [
  "proc-macro2",
  "quote",
- "syn 2.0.119",
+ "syn 3.0.4",
 ]
 
 [[package]]
 name = "futures-sink"
-version = "0.3.33"
+version = "0.3.34"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "e34418ac499d6305c2fb5ad0ed2f6ac998c5f8ca209b4510f7f94242c647e307"
+checksum = "1944426bf7d03f1d14f708785e4b33efd750b36d48a157b836b3efc15ede8e1d"
 
 [[package]]
 name = "futures-task"
-version = "0.3.33"
+version = "0.3.34"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "b231ed28831efb4a61a08580c4bc233ec56bc009f4cd8f52da2c3cb97df0c109"
+checksum = "cd417de3d1d015fc3bfd2b1ea46dfc7bab72ef86f1cc7cc9c78e728b34a6d1fd"
 
 [[package]]
 name = "futures-util"
-version = "0.3.33"
+version = "0.3.34"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "a77a90a256fce34da66415271e30f94ee91c57b04b8a2c042d9cf3220179deaa"
+checksum = "0d50a92467f8ba5dd6e3ee5d4bd04d73ab2e4e1c44474a0674821dfce14b79bc"
 dependencies = [
  "futures-core",
  "futures-io",
@@ -1397,9 +1429,9 @@ dependencies = [
 
 [[package]]
 name = "glob"
-version = "0.3.3"
+version = "0.3.4"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "0cc23270f6e1808e30a928bdc84dea0b9b4136a8bc82338574f23baf47bbd280"
+checksum = "e4eba85ea1d0a966a983acd07deee566e67395d2d96b6fb39e62b5a833f1eb0b"
 
 [[package]]
 name = "gobject-sys"
@@ -1521,9 +1553,9 @@ dependencies = [
 
 [[package]]
 name = "http"
-version = "1.4.2"
+version = "1.5.0"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "6970f50e31d6fc17d3fa27329444bfa74e196cf62e95052a3f6fee181dba6425"
+checksum = "918d3568bebf352712bc2ef3d46a8bcf1a75b373be6539de198e9105cbbf9ce0"
 dependencies = [
  "bytes",
  "itoa",
@@ -1541,9 +1573,9 @@ dependencies = [
 
 [[package]]
 name = "http-body-util"
-version = "0.1.4"
+version = "0.1.5"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "e9f41fd6a08e4d4ec69df65976da761afd5ad5e58a9d4acb46bd1c953a9e3ff2"
+checksum = "23169fe34a5fbcdd3f3862e78fb9b6fccd5f02a6dc6f732547005d45631ce71c"
 dependencies = [
  "bytes",
  "futures-core",
@@ -1560,9 +1592,9 @@ checksum = "6dbf3de79e51f3d586ab4cb9d5c3e2c14aa28ed23d180cf89b4df0454a69cc87"
 
 [[package]]
 name = "hyper"
-version = "1.10.1"
+version = "1.11.1"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "55281c53a1894c864990125767da440a4e630446785086f52523b20033b74498"
+checksum = "27b501faa50e7a26c3d3560ca625132f4078a17771f4810baf70475ae48cbe43"
 dependencies = [
  "atomic-waker",
  "bytes",
@@ -1654,9 +1686,9 @@ dependencies = [
 
 [[package]]
 name = "icu_collections"
-version = "2.2.0"
+version = "2.3.0"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "2984d1cd16c883d7935b9e07e44071dca8d917fd52ecc02c04d5fa0b5a3f191c"
+checksum = "fa68d21081c4a05d5a901a1c62add574c77048b6a1c67be3b50ce0b60d4ca513"
 dependencies = [
  "displaydoc",
  "potential_utf",
@@ -1668,9 +1700,9 @@ dependencies = [
 
 [[package]]
 name = "icu_locale_core"
-version = "2.2.0"
+version = "2.3.0"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "92219b62b3e2b4d88ac5119f8904c10f8f61bf7e95b640d25ba3075e6cac2c29"
+checksum = "d56e28588da92eee5c3201a6eff33fabdd49b62269c8938d4ff050ce4d900deb"
 dependencies = [
  "displaydoc",
  "litemap",
@@ -1681,9 +1713,9 @@ dependencies = [
 
 [[package]]
 name = "icu_normalizer"
-version = "2.2.0"
+version = "2.3.0"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "c56e5ee99d6e3d33bd91c5d85458b6005a22140021cc324cea84dd0e72cff3b4"
+checksum = "12f9cf5f235641ed274641dd81c3f28d870e276763d0797aeeab72317b1c646f"
 dependencies = [
  "icu_collections",
  "icu_normalizer_data",
@@ -1695,16 +1727,17 @@ dependencies = [
 
 [[package]]
 name = "icu_normalizer_data"
-version = "2.2.0"
+version = "2.3.0"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "da3be0ae77ea334f4da67c12f149704f19f81d1adf7c51cf482943e84a2bad38"
+checksum = "1563da1ed3e0b3bf3d74c9b85917ac9c56464d2f57242270c09c9e752f8021a0"
 
 [[package]]
 name = "icu_properties"
-version = "2.2.0"
+version = "2.3.0"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "bee3b67d0ea5c2cca5003417989af8996f8604e34fb9ddf96208a033901e70de"
+checksum = "7e7ca276ad3145661a65914e6daf131ca5120cd3dcee8f8f3214b8875184a148"
 dependencies = [
+ "displaydoc",
  "icu_collections",
  "icu_locale_core",
  "icu_properties_data",
@@ -1715,15 +1748,15 @@ dependencies = [
 
 [[package]]
 name = "icu_properties_data"
-version = "2.2.0"
+version = "2.3.0"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "8e2bbb201e0c04f7b4b3e14382af113e17ba4f63e2c9d2ee626b720cbce54a14"
+checksum = "e590f038c1464a96894fd6d10127e90a8be4509f56ff7ecef851b15cee0b7caa"
 
 [[package]]
 name = "icu_provider"
-version = "2.2.0"
+version = "2.3.1"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "139c4cf31c8b5f33d7e199446eff9c1e02decfc2f0eec2c8d71f65befa45b421"
+checksum = "d27bbb9d3abbefac45d55f647c9de1d44aafcd1186eb91879afef17c396c3e73"
 dependencies = [
  "displaydoc",
  "icu_locale_core",
@@ -1774,9 +1807,9 @@ dependencies = [
 
 [[package]]
 name = "indexmap"
-version = "2.14.0"
+version = "2.14.1"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "d466e9454f08e4a911e14806c24e16fba1b4c121d1ea474396f396069cf949d9"
+checksum = "07aa2048142242915a31d35844fb311e0e53fcca590c3a0a40dcf1b841fa09eb"
 dependencies = [
  "equivalent",
  "hashbrown 0.17.1",
@@ -1795,9 +1828,9 @@ dependencies = [
 
 [[package]]
 name = "ipnet"
-version = "2.12.0"
+version = "2.12.1"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "d98f6fed1fde3f8c21bc40a1abb88dd75e67924f9cffc3ef95607bad8017f8e2"
+checksum = "6a756c3fac73139e83f14c2d742155dd2b78d3ee56597b419a0579b7bdd6dd78"
 
 [[package]]
 name = "is_terminal_polyfill"
@@ -1834,6 +1867,59 @@ dependencies = [
  "system-deps",
 ]
 
+[[package]]
+name = "jiff"
+version = "0.2.35"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "668b7183bd07af9a4885f5c35b0cc5c83c4607a913c16b7e17291832910d2dcc"
+dependencies = [
+ "defmt",
+ "jiff-core",
+ "jiff-static",
+ "jiff-tzdb-platform",
+ "log",
+ "portable-atomic",
+ "portable-atomic-util",
+ "serde_core",
+ "windows-link 0.2.1",
+]
+
+[[package]]
+name = "jiff-core"
+version = "0.1.0"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "7feca88439efe53da3754500c1851dedf3cb36c524dd5cf8225cc0794de95d09"
+dependencies = [
+ "defmt",
+]
+
+[[package]]
+name = "jiff-static"
+version = "0.2.35"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "3a69dcb3a21cfb32ce1cd056169337ca284af0766dd766e7878819b251a49204"
+dependencies = [
+ "jiff-core",
+ "proc-macro2",
+ "quote",
+ "syn 2.0.119",
+]
+
+[[package]]
+name = "jiff-tzdb"
+version = "0.1.8"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "142bd39932ad231f10513df9ab62661fead8719872150b7ad02a2df79f4e141e"
+
+[[package]]
+name = "jiff-tzdb-platform"
+version = "0.1.3"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "875a5a69ac2bab1a891711cf5eccbec1ce0341ea805560dcd90b7a2e925132e8"
+dependencies = [
+ "jiff-tzdb",
+]
+
 [[package]]
 name = "jni"
 version = "0.21.1"
@@ -1910,9 +1996,9 @@ dependencies = [
 
 [[package]]
 name = "js-sys"
-version = "0.3.103"
+version = "0.3.104"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "53b44bfcdb3f8d5837a46dae1ca9660a837176eee74a28b229bc626816589102"
+checksum = "0e0c1080212aad755ea003d18543e8768dd432c48819efd73a7bf1e39b7a5a3a"
 dependencies = [
  "cfg-if",
  "futures-util",
@@ -1988,9 +2074,9 @@ dependencies = [
 
 [[package]]
 name = "libc"
-version = "0.2.186"
+version = "0.2.189"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "68ab91017fe16c622486840e4c83c9a37afeff978bd239b5293d61ece587de66"
+checksum = "3eaf3ede3fee6db1a4c2ee091bf8a8b4dccdc6d17f656fb07896ee72867612f2"
 
 [[package]]
 name = "libdbus-sys"
@@ -2013,9 +2099,9 @@ dependencies = [
 
 [[package]]
 name = "libredox"
-version = "0.1.18"
+version = "0.1.23"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "c943259e342f1e06ff2da7a83eabdfe7f92ce10262688dbf1895ff0b3e6e4652"
+checksum = "8d8f1ea3f21fd3405dcaf6c9b5c1630af9afc422d9073ea39c5f6d6c772e08ed"
 dependencies = [
  "libc",
 ]
@@ -2028,9 +2114,9 @@ checksum = "32a66949e030da00e8c7d4434b251670a91556f4144941d37452769c25d58a53"
 
 [[package]]
 name = "litemap"
-version = "0.8.2"
+version = "0.8.3"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "92daf443525c4cce67b150400bc2316076100ce0b3686209eb8cf3c31612e6f0"
+checksum = "47d9d19d1d6efa0109d2f65ff4c85cddd50bd572e5a00127ab10987290bcefae"
 
 [[package]]
 name = "lock_api"
@@ -2043,9 +2129,9 @@ dependencies = [
 
 [[package]]
 name = "log"
-version = "0.4.33"
+version = "0.4.34"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "0ceec5bc11778974d1bcb055b18002eba7f4b3518b6a0081b3af5f21666da9ad"
+checksum = "f9f8bd3e56ce4dfc153cf470fffbfa98c7620958b312ca5c3a4b8d5181fd13c6"
 
 [[package]]
 name = "markup5ever"
@@ -2104,11 +2190,21 @@ dependencies = [
  "simd-adler32",
 ]
 
+[[package]]
+name = "miniz_oxide"
+version = "0.9.1"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "b63fbc4a50860e98e7b2aa7804ded1db5cbc3aff9193adaff57a6931bf7c4b4c"
+dependencies = [
+ "adler2",
+ "simd-adler32",
+]
+
 [[package]]
 name = "mio"
-version = "1.2.2"
+version = "1.2.3"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "30d65c71f1ce40ab09135ce117d742b9f8a19ff91a41a8b57ed50bc2de59c427"
+checksum = "4b18443e9c262bfe8fa82f51666e2642c53393f7e5c27b3e1aeab922cff5b9d8"
 dependencies = [
  "libc",
  
```

## Advisories after the update

```
    Fetching advisory database from `https://github.com/RustSec/advisory-db.git`
      Loaded 1239 security advisories (from /home/runner/.cargo/advisory-db)
    Updating crates.io index
    Scanning Cargo.lock for vulnerabilities (521 crate dependencies)
Crate:     atk
Version:   0.18.2
Warning:   unmaintained
Title:     gtk-rs GTK3 bindings - no longer maintained
Date:      2024-03-04
ID:        RUSTSEC-2024-0413
URL:       https://rustsec.org/advisories/RUSTSEC-2024-0413

Crate:     atk-sys
Version:   0.18.2
Warning:   unmaintained
Title:     gtk-rs GTK3 bindings - no longer maintained
Date:      2024-03-04
ID:        RUSTSEC-2024-0416
URL:       https://rustsec.org/advisories/RUSTSEC-2024-0416

Crate:     gdk
Version:   0.18.2
Warning:   unmaintained
Title:     gtk-rs GTK3 bindings - no longer maintained
Date:      2024-03-04
ID:        RUSTSEC-2024-0412
URL:       https://rustsec.org/advisories/RUSTSEC-2024-0412

Crate:     gdk-sys
Version:   0.18.2
Warning:   unmaintained
Title:     gtk-rs GTK3 bindings - no longer maintained
Date:      2024-03-04
ID:        RUSTSEC-2024-0418
URL:       https://rustsec.org/advisories/RUSTSEC-2024-0418

Crate:     gdkwayland-sys
Version:   0.18.2
Warning:   unmaintained
Title:     gtk-rs GTK3 bindings - no longer maintained
Date:      2024-03-04
ID:        RUSTSEC-2024-0411
URL:       https://rustsec.org/advisories/RUSTSEC-2024-0411

Crate:     gdkx11
Version:   0.18.2
Warning:   unmaintained
Title:     gtk-rs GTK3 bindings - no longer maintained
Date:      2024-03-04
ID:        RUSTSEC-2024-0417
URL:       https://rustsec.org/advisories/RUSTSEC-2024-0417

Crate:     gdkx11-sys
Version:   0.18.2
Warning:   unmaintained
Title:     gtk-rs GTK3 bindings - no longer maintained
Date:      2024-03-04
ID:        RUSTSEC-2024-0414
URL:       https://rustsec.org/advisories/RUSTSEC-2024-0414

Crate:     gtk
Version:   0.18.2
Warning:   unmaintained
Title:     gtk-rs GTK3 bindings - no longer maintained
Date:      2024-03-04
ID:        RUSTSEC-2024-0415
URL:       https://rustsec.org/advisories/RUSTSEC-2024-0415

Crate:     gtk-sys
Version:   0.18.2
Warning:   unmaintained
Title:     gtk-rs GTK3 bindings - no longer maintained
Date:      2024-03-04
ID:        RUSTSEC-2024-0420
URL:       https://rustsec.org/advisories/RUSTSEC-2024-0420

Crate:     gtk3-macros
Version:   0.18.2
Warning:   unmaintained
Title:     gtk-rs GTK3 bindings - no longer maintained
Date:      2024-03-04
ID:        RUSTSEC-2024-0419
URL:       https://rustsec.org/advisories/RUSTSEC-2024-0419

Crate:     proc-macro-error
Version:   1.0.4
Warning:   unmaintained
Title:     proc-macro-error is unmaintained
Date:      2024-09-01
ID:        RUSTSEC-2024-0370
URL:       https://rustsec.org/advisories/RUSTSEC-2024-0370

Crate:     unic-char-property
Version:   0.9.0
Warning:   unmaintained
Title:     `unic-char-property` is unmaintained
Date:      2025-10-18
ID:        RUSTSEC-2025-0081
URL:       https://rustsec.org/advisories/RUSTSEC-2025-0081

Crate:     unic-char-range
Version:   0.9.0
Warning:   unmaintained
Title:     `unic-char-range` is unmaintained
Date:      2025-10-18
ID:        RUSTSEC-2025-0075
URL:       https://rustsec.org/advisories/RUSTSEC-2025-0075

Crate:     unic-common
Version:   0.9.0
Warning:   unmaintained
Title:     `unic-common` is unmaintained
Date:      2025-10-18
ID:        RUSTSEC-2025-0080
URL:       https://rustsec.org/advisories/RUSTSEC-2025-0080

Crate:     unic-ucd-ident
Version:   0.9.0
Warning:   unmaintained
Title:     `unic-ucd-ident` is unmaintained
Date:      2025-10-18
ID:        RUSTSEC-2025-0100
URL:       https://rustsec.org/advisories/RUSTSEC-2025-0100

Crate:     unic-ucd-version
Version:   0.9.0
Warning:   unmaintained
Title:     `unic-ucd-version` is unmaintained
Date:      2025-10-18
ID:        RUSTSEC-2025-0098
URL:       https://rustsec.org/advisories/RUSTSEC-2025-0098

Crate:     glib
Version:   0.18.5
Warning:   unsound
Title:     Unsoundness in `Iterator` and `DoubleEndedIterator` impls for `glib::VariantStrIter`
Date:      2024-03-30
ID:        RUSTSEC-2024-0429
URL:       https://rustsec.org/advisories/RUSTSEC-2024-0429

warning: 17 allowed warnings found

```

<sub>Opened by the Dependency auto-fix workflow. Both blocks are truncated at
the size GitHub will render; run the commands locally for the full output.</sub>
