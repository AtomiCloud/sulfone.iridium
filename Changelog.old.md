## [2.2.1](https://github.com/AtomiCloud/sulfone.iridium/compare/v2.2.0...v2.2.1) (2025-06-25)


### 🐛 Bug Fixes 🐛

* **bollard:** resolve dependency issue with bollard upgrade ([3b17412](https://github.com/AtomiCloud/sulfone.iridium/commit/3b174122abeb156a0e60c71649d961d9cac8610c))

## [2.2.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v2.1.0...v2.2.0) (2025-06-23)


### ✨ Features ✨

* **http client:** add 3 endpoints to retrieve full version data ([18616c9](https://github.com/AtomiCloud/sulfone.iridium/commit/18616c972957452bf401f7a8d93cba9e72c82c00))
* **daemon:** allow choosing cyanprint coord daemon port ([c5a5599](https://github.com/AtomiCloud/sulfone.iridium/commit/c5a55998c5eeb0dcfbca679c028aaf70a8970f13))
* **default:** allow empty model ([661bf5a](https://github.com/AtomiCloud/sulfone.iridium/commit/661bf5ac7e0ba67a6bc52beff517d2b1a534d5c6))
* allow registry and coordinator be set by env ([183b442](https://github.com/AtomiCloud/sulfone.iridium/commit/183b4422437e8195571f70a0560dde32f33f93c1))


### 🐛 Bug Fixes 🐛

* allow non-specification of templates/processor/plugins versions ([2502428](https://github.com/AtomiCloud/sulfone.iridium/commit/25024280e934da772ea5c8811147a84cce52b8f8))
* **default:** ensure starting daemon is idempotent ([ce885f3](https://github.com/AtomiCloud/sulfone.iridium/commit/ce885f3d130007b4c9c0467db9b7e2bd3987e112))


### 🧪 Tests 🧪

* initial setup for e2e for publishing artifacts ([69357d1](https://github.com/AtomiCloud/sulfone.iridium/commit/69357d18a5006f24a0790a7644aa871a7412d7bf))

## [2.1.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v2.0.0...v2.1.0) (2025-05-10)


### ✨ Features ✨

* **default:** allow interactive updates ([dad92d1](https://github.com/AtomiCloud/sulfone.iridium/commit/dad92d14f413c4f7875d6adba43425bd04240f27))
* **update:** better choice formatting ([13d479d](https://github.com/AtomiCloud/sulfone.iridium/commit/13d479dae5d54807b9a03a8372db8c9a9d20ae25))
* **update:** new commands to upgrade project's template ([d0f8ce6](https://github.com/AtomiCloud/sulfone.iridium/commit/d0f8ce6ee51c7a5408a97cd924b7cd315c5a645c))

## [2.0.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v1.10.0...v2.0.0) (2025-05-07)


### ✨ Features ✨

* **breaking:** release v2 ([87c6394](https://github.com/AtomiCloud/sulfone.iridium/commit/87c63941d948ebb34ffca63628478d28ee26d648))

## [1.10.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v1.9.3...v1.10.0) (2025-05-06)


### ✨ Features ✨

* allow for update, re-run and new ([b29b534](https://github.com/AtomiCloud/sulfone.iridium/commit/b29b53434e7efc1b1abbf10e008477e28ad46f25))
* debug flag ([77b3ce0](https://github.com/AtomiCloud/sulfone.iridium/commit/77b3ce0559ff58b625a50b5c34c66856e9e3037b))
* store template generation metadata in .cyan_state.yaml ([17533a5](https://github.com/AtomiCloud/sulfone.iridium/commit/17533a5392ce046af72601626167ac3f07621e04))
* use username instead of user_id in template metadata ([9f16d34](https://github.com/AtomiCloud/sulfone.iridium/commit/9f16d3479dfb3272d32d3a6c762c85269c806a42))


### 🐛 Bug Fixes 🐛

* add serde tagging to Answer enum to fix serialization ([5a3aa82](https://github.com/AtomiCloud/sulfone.iridium/commit/5a3aa8286e199e7f13c6be97f46ecc1a20e555e4))

## [1.9.3](https://github.com/AtomiCloud/sulfone.iridium/compare/v1.9.2...v1.9.3) (2025-05-04)


### 🐛 Bug Fixes 🐛

* try to force musl target for static compile ([e758ded](https://github.com/AtomiCloud/sulfone.iridium/commit/e758ded09b29cd0aa1124c2f54b43d54f74cdef1))
* try using static builds ([aa895e3](https://github.com/AtomiCloud/sulfone.iridium/commit/aa895e3377972da79e24634aa8075124a76d47cf))
* try using static builds ([#29](https://github.com/AtomiCloud/sulfone.iridium/issues/29)) ([b94f293](https://github.com/AtomiCloud/sulfone.iridium/commit/b94f2930f39ea64ae2abb45bb459df4486ee9f36))

## [1.9.2](https://github.com/AtomiCloud/sulfone.iridium/compare/v1.9.1...v1.9.2) (2025-05-04)


### 🐛 Bug Fixes 🐛

* incorrect bin name ([1417621](https://github.com/AtomiCloud/sulfone.iridium/commit/14176213fcbb71fb95c548c45574dcf2909e9966))
* remove cargo cache in hopes to fix build errors ([6484746](https://github.com/AtomiCloud/sulfone.iridium/commit/6484746d3ced9db9136e53d8998acc56860008e9))
* static build ([5b00830](https://github.com/AtomiCloud/sulfone.iridium/commit/5b008307d257cb7c8469fa0a75a6351b3c2bf7f9))
* use nix build process ([107c8c3](https://github.com/AtomiCloud/sulfone.iridium/commit/107c8c3c83c8670caf2315d7c7322d31892d5215))

## [1.9.1](https://github.com/AtomiCloud/sulfone.iridium/compare/v1.9.0...v1.9.1) (2025-05-04)


### 🐛 Bug Fixes 🐛

* ensure publish finished after build ([dbd8e3d](https://github.com/AtomiCloud/sulfone.iridium/commit/dbd8e3db81cb0868018ffc56d85a9ebac6e742a3))

## [1.9.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v1.8.0...v1.9.0) (2025-05-03)


### ✨ Features ✨

* use 3 way merge instead of just writing to file system ([475c7ed](https://github.com/AtomiCloud/sulfone.iridium/commit/475c7ede8bb0fd8fc06feb04d783bba7eec49c97))


### 🐛 Bug Fixes 🐛

* ignore incremental changelog that prevents release ([4ca20c8](https://github.com/AtomiCloud/sulfone.iridium/commit/4ca20c8122b70548e68bd56ca0ba911646c7cc19))

## [1.8.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v1.7.0...v1.8.0) (2025-05-02)


### ✨ Features ✨

* **breaking:** [CU 86et87kzu] CyanPrint referenced by id ([#26](https://github.com/AtomiCloud/sulfone.iridium/issues/26)) ([da23293](https://github.com/AtomiCloud/sulfone.iridium/commit/da23293c537d40a53668a1af694dd5dd27001f00))
* upgrade all dependencies to latest ([01e4159](https://github.com/AtomiCloud/sulfone.iridium/commit/01e4159c83feea65cdf0573997b7a865bd3c50cc))
* **breaking:** use answer referenced by ID ([6ec65f0](https://github.com/AtomiCloud/sulfone.iridium/commit/6ec65f0ca0bf1f985d8faf930bb74a0b338d0874))


### 🐛 Bug Fixes 🐛

* incorrect ci environment -- move to fenix rust ([9b25df8](https://github.com/AtomiCloud/sulfone.iridium/commit/9b25df8917712a87da5bfe6569519d81009cc07a))
* increase timeout of merge to 20min ([f735ef5](https://github.com/AtomiCloud/sulfone.iridium/commit/f735ef5e539efd30b32e1c35344214f5ba0abd37))
* linting errors from clippy ([36b28d4](https://github.com/AtomiCloud/sulfone.iridium/commit/36b28d458600cc55b09f087cf239a6711a2f79d8))
* treefmt in hooks ([7752a93](https://github.com/AtomiCloud/sulfone.iridium/commit/7752a935aeda3cb6c3069657a632e6bf21a9466c))

## [1.7.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v1.6.1...v1.7.0) (2025-04-26)


### ✨ Features ✨

* upgrade all packages ([ae030fd](https://github.com/AtomiCloud/sulfone.iridium/commit/ae030fd19eb75e69b599e28837fb125bed37e494))
* upgrade infra configuration ([943bedf](https://github.com/AtomiCloud/sulfone.iridium/commit/943bedfb897b005a9b97e91915d17533343d30c7))
* upgrade infrastructure ([7aafff8](https://github.com/AtomiCloud/sulfone.iridium/commit/7aafff8d5ceaef5cb5b67dc73cea63f19e4a94ab))


### 🐛 Bug Fixes 🐛

* deprecate windows support ([4a72573](https://github.com/AtomiCloud/sulfone.iridium/commit/4a725733fd7675f722048f5876607c00fee965b8))
* incorrect release yaml ([b608aac](https://github.com/AtomiCloud/sulfone.iridium/commit/b608aac06bae6d6dfc5deb93edc720c49c44c6a8))
* pinning cross ([3071410](https://github.com/AtomiCloud/sulfone.iridium/commit/30714106e0d963948fa97c1ca112cda4ced2d885))
* **ci:** upgrade actions ([cec6869](https://github.com/AtomiCloud/sulfone.iridium/commit/cec6869ca1a6124d43a9aad14ff678eef377903a))
* use different os to build ([886e2b1](https://github.com/AtomiCloud/sulfone.iridium/commit/886e2b184df33a2c731e33cde7201bec4055e067))
* use macos runner ([70c7941](https://github.com/AtomiCloud/sulfone.iridium/commit/70c79417ad5a53e179288f76f2fe6db56dd2ed16))

## [1.6.1](https://github.com/AtomiCloud/sulfone.iridium/compare/v1.6.0...v1.6.1) (2025-01-28)


### 🐛 Bug Fixes 🐛

* update goreleaser config ([88311e8](https://github.com/AtomiCloud/sulfone.iridium/commit/88311e853d7aed7ee0a0b28442ca7b29a438b2c9))

## [1.6.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v1.5.0...v1.6.0) (2025-01-28)


### ✨ Features ✨

* upgrade to 1.84.0 rust ([b4d6845](https://github.com/AtomiCloud/sulfone.iridium/commit/b4d6845d1874560fcabaea3a039f31ebf6ece360))


### 🐛 Bug Fixes 🐛

* pin to v3 cargo lock ([43efc33](https://github.com/AtomiCloud/sulfone.iridium/commit/43efc3378c9f6d98d63f7b25d877b8fa98d7875b))

## [1.5.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v1.4.0...v1.5.0) (2025-01-28)


### ✨ Features ✨

* nix pin to new nix-registry ([e3746be](https://github.com/AtomiCloud/sulfone.iridium/commit/e3746be814bebe2164c6006ee94820977898e7b2))


### 🐛 Bug Fixes 🐛

* release script pin to npm ([eab1563](https://github.com/AtomiCloud/sulfone.iridium/commit/eab1563908726e9560cf61cfbeddc5bfac958556))

## [1.4.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v1.3.0...v1.4.0) (2023-11-14)


### ✨ Features ✨

* YUM repository ([44016ce](https://github.com/AtomiCloud/sulfone.iridium/commit/44016ce8703c77af8db5fb2881ce662826b9fd7d))

## [1.3.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v1.2.0...v1.3.0) (2023-11-13)


### ✨ Features ✨

* local coordinator setup ([b8853eb](https://github.com/AtomiCloud/sulfone.iridium/commit/b8853eba3b5c358429952f7529fb7b9db8b66f36))

## [1.2.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v1.1.0...v1.2.0) (2023-11-13)


### ✨ Features ✨

* read token from ENV ([1c687ce](https://github.com/AtomiCloud/sulfone.iridium/commit/1c687ce03f6171b211ae23fb06e6db5d7cb80770))

## [1.1.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v1.0.3...v1.1.0) (2023-11-11)


### ✨ Features ✨

* migrate to tag-based images ([3f329c2](https://github.com/AtomiCloud/sulfone.iridium/commit/3f329c2ce55b03093d401f88005e63526e49a7ec))

## [1.0.3](https://github.com/AtomiCloud/sulfone.iridium/compare/v1.0.2...v1.0.3) (2023-11-08)


### 🐛 Bug Fixes 🐛

* incorrect build for linux system ([07500b3](https://github.com/AtomiCloud/sulfone.iridium/commit/07500b3f18dd5ce77087cf4dd3ba130a064764d9))

## [1.0.2](https://github.com/AtomiCloud/sulfone.iridium/compare/v1.0.1...v1.0.2) (2023-11-08)


### 🐛 Bug Fixes 🐛

* nix configuration for installation ([80831c6](https://github.com/AtomiCloud/sulfone.iridium/commit/80831c6663fd9ff5390b3de1f7990bcc5a605f1c))

## [1.0.1](https://github.com/AtomiCloud/sulfone.iridium/compare/v1.0.0...v1.0.1) (2023-11-08)


### 🐛 Bug Fixes 🐛

* linux packaging ([269cc6c](https://github.com/AtomiCloud/sulfone.iridium/commit/269cc6c67b201afe10f340be23cf55ea97c16b42))

## 1.0.0 (2023-11-08)


### ✨ Features ✨

* initial commit ([d51d91a](https://github.com/AtomiCloud/sulfone.iridium/commit/d51d91a2bc32f3d4855e9546395340ec1fa7137e))
