## [2.19.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v2.18.1...v2.19.0) (2026-03-25)


### 📜 Documentation 📜

* **CU-86ex0ycve:** add spec and plans for preset answers ([45ba6de](https://github.com/AtomiCloud/sulfone.iridium/commit/45ba6debbd1e9e4a8fb9c161be719fb144abe799))


### ✨ Features ✨

* **cyancoordinator:** dep resolution injection for preset answers ([9e0dc57](https://github.com/AtomiCloud/sulfone.iridium/commit/9e0dc577ccbdb5646e2f113484544b0c4bca48c7))
* **cyanregistry:** extend data pipeline for preset answers ([5ab99d3](https://github.com/AtomiCloud/sulfone.iridium/commit/5ab99d39afbce7bed5b3d2f56253e3591128bf5e))


### 🐛 Bug Fixes 🐛

* **cyancoordinator:** clarify test comments for dependency traversal ([65e7a4c](https://github.com/AtomiCloud/sulfone.iridium/commit/65e7a4cc7f9dffef9d89ff59f439e578d76819dc))


### 🧪 Tests 🧪

* **e2e:** add E2E test coverage for preset answers feature ([0a3726b](https://github.com/AtomiCloud/sulfone.iridium/commit/0a3726bb9a69a650910bfc835bc9669af2d5d47d))

## [2.18.1](https://github.com/AtomiCloud/sulfone.iridium/compare/v2.18.0...v2.18.1) (2026-03-24)


### 🐛 Bug Fixes 🐛

* **e2e:** update E2E test snapshots and add ticket specification ([db62b58](https://github.com/AtomiCloud/sulfone.iridium/commit/db62b58d72ea57c00b98fa8fa985ba9c10cb3f38))

## [2.18.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v2.17.0...v2.18.0) (2026-03-23)


### 📜 Documentation 📜

* **CU-86ewz23zw:** add v2 spec and plans for e2e test expansion ([861da7a](https://github.com/AtomiCloud/sulfone.iridium/commit/861da7a11d5d92a852680630c871c29366a32190))


### ✨ Features ✨

* **e2e:** add template6 nested template generator ([1b54593](https://github.com/AtomiCloud/sulfone.iridium/commit/1b54593a03634228924aa4fe1dd021d444cf8cb6))
* add test specs for port allocation race condition fix ([a7f8ec9](https://github.com/AtomiCloud/sulfone.iridium/commit/a7f8ec9094b8283c528a2c0daf31cea0730cd78d))
* fix port allocation race condition across all testtry commands ([f78e823](https://github.com/AtomiCloud/sulfone.iridium/commit/f78e823ba0dbaf83bd59ee2e77a0305044ce311f))
* **e2e:** fix snapshot mismatches and finalize local/full test phases ([19f7ada](https://github.com/AtomiCloud/sulfone.iridium/commit/19f7adab9804584ceac578a947b80830bd16b3d7))
* **e2e:** populate full.sh with create/upgrade/conflict/resolver tests ([2bffca7](https://github.com/AtomiCloud/sulfone.iridium/commit/2bffca73e5667b255f3abd3ccd3292a029727503))
* **e2e:** populate local.sh with test/try/stress commands ([b384412](https://github.com/AtomiCloud/sulfone.iridium/commit/b3844124228975a9dbe1891af4a0646647c0423b))
* **e2e:** split e2e.sh into build/local/full phases ([9ad4c66](https://github.com/AtomiCloud/sulfone.iridium/commit/9ad4c66c570d100bcc0290139af8382e69288afa))


### 🐛 Bug Fixes 🐛

* address CodeRabbit review feedback ([080a7c3](https://github.com/AtomiCloud/sulfone.iridium/commit/080a7c3b1f362b5d41e36ffa983d57bcd4d4adea))
* **e2e:** disable SC1090 shellcheck warning for dynamic source ([4ccdf46](https://github.com/AtomiCloud/sulfone.iridium/commit/4ccdf46027cbb7e8fea854a555fbe530273590f2))
* **pre-commit:** exclude e2e fixture files from treefmt ([874d298](https://github.com/AtomiCloud/sulfone.iridium/commit/874d298122369d3b901bf9c9d34d45aa02567a8f))
* **cyanprint:** generate fresh container name per retry attempt ([89aab96](https://github.com/AtomiCloud/sulfone.iridium/commit/89aab96e1a8c4c50081c043ffbe6fabf9efe2588))
* **e2e:** update test.cyan.yaml with correct snapshot paths ([3dad3a4](https://github.com/AtomiCloud/sulfone.iridium/commit/3dad3a4f9f62b12dbc4ce82c8cfb5abbc126524e))
* **test:** use dynamic port range in random_then_sequential test ([827fd82](https://github.com/AtomiCloud/sulfone.iridium/commit/827fd82e1c2cb6e475c07bed989813006865ffc3))

## [2.17.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v2.16.0...v2.17.0) (2026-03-17)


### 📜 Documentation 📜

* add implementation plans for CU-86ewynyxu v1 ([ec9705e](https://github.com/AtomiCloud/sulfone.iridium/commit/ec9705e8b460d7c822336c154f56bcf9be1a1e7d))
* add task spec for CU-86ewynyxu ([c8c0d36](https://github.com/AtomiCloud/sulfone.iridium/commit/c8c0d364dbb03948ff9d77a0ab6fe803e8597835))


### ✨ Features ✨

* **cyanprint:** implement run-scoped container ownership ([f82ac36](https://github.com/AtomiCloud/sulfone.iridium/commit/f82ac36f2dfe2f3f76076a0dc816c8f5b5273b88))

## [2.16.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v2.15.1...v2.16.0) (2026-03-17)


### 📜 Documentation 📜

* add implementation plans for CU-86ewyng87 v1 ([891815a](https://github.com/AtomiCloud/sulfone.iridium/commit/891815a7e25eec684173a4792ca9f65f69794449))
* add task spec for CU-86ewyng87 ([6c93c0c](https://github.com/AtomiCloud/sulfone.iridium/commit/6c93c0ceeaa6b95c1f3c3e5dc25ede7814a748e0))


### ✨ Features ✨

* **commands:** update coordinator endpoint for test/try commands ([ce37040](https://github.com/AtomiCloud/sulfone.iridium/commit/ce37040f1c3cd559a5cba669637ec76c1e96b40b))

## [2.15.1](https://github.com/AtomiCloud/sulfone.iridium/compare/v2.15.0...v2.15.1) (2026-03-16)


### 🐛 Bug Fixes 🐛

* **cyanprint:** move try_setup back to per-test-case flow [CU-86ewy8vg8] ([903db77](https://github.com/AtomiCloud/sulfone.iridium/commit/903db7755c625e39a1cc69c3b316567c53c3bafa))

## [2.15.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v2.14.0...v2.15.0) (2026-03-15)


### 📜 Documentation 📜

* add task spec for CU-86ewy2qyy ([fbb22ef](https://github.com/AtomiCloud/sulfone.iridium/commit/fbb22ef6e863623b59877a51df048df1c9a3343e))


### ✨ Features ✨

* **cyanregistry:** add env var substitution for configs [CU-86ewy2qyy] ([2dc828e](https://github.com/AtomiCloud/sulfone.iridium/commit/2dc828ee50bb2a64ac244a4486fb3088cfcd886d))

## [2.14.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v2.13.0...v2.14.0) (2026-03-15)


### 📜 Documentation 📜

* add docs reqs and critical fixes to plans ([09745f0](https://github.com/AtomiCloud/sulfone.iridium/commit/09745f0530c191edc3ed40d644a8d7e6f9853ce6))
* add implementation plans for CU-86ewvp51k v1 ([a523d0b](https://github.com/AtomiCloud/sulfone.iridium/commit/a523d0b36ce5f0126ac9058716bf74e757ddbc0b))
* add task spec for CU-86ewvp51k [Ir] Test Command ([744d66a](https://github.com/AtomiCloud/sulfone.iridium/commit/744d66a8e6980aa0f7cef96c667d7d9746a2e5fa))


### ✨ Features ✨

* **cyanprint:** add processor/plugin/resolver tests [CU-86ewvp51k] ([e7ddd2c](https://github.com/AtomiCloud/sulfone.iridium/commit/e7ddd2c2f4fda28fac1a82a9804fffd1203599ab))
* **cyanprint:** add test init tree walk [CU-86ewvp51k] ([a440cbe](https://github.com/AtomiCloud/sulfone.iridium/commit/a440cbee53b2208c00d53ec8ab93cdcbf35cee02))
* **cyanprint:** add test template command [CU-86ewvp51k] ([99b3ba8](https://github.com/AtomiCloud/sulfone.iridium/commit/99b3ba8d7ca12ba1a84a17a894dcd0eecae7ed76))


### 🐛 Bug Fixes 🐛

* address all CodeRabbit review findings [CU-86ewvp51k] ([329a624](https://github.com/AtomiCloud/sulfone.iridium/commit/329a624eacbcb212ae51e6f4debdc038cec1a6cb))
* address CodeRabbit review round 6 findings [CU-86ewvp51k] ([cf1b1fe](https://github.com/AtomiCloud/sulfone.iridium/commit/cf1b1febbb1aa158a328954bf178b25bb49ff0b4))
* address prereview findings [CU-86ewvp51k] ([be1f64e](https://github.com/AtomiCloud/sulfone.iridium/commit/be1f64e10c366d18c96d2a649bf88d3de03fa099))

## [2.13.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v2.12.0...v2.13.0) (2026-03-12)


### 📜 Documentation 📜

* add implementation plan for CU-86ewvp51j v1 ([76da333](https://github.com/AtomiCloud/sulfone.iridium/commit/76da333e45bab9b2c13eaa97b389093dda00ddbc))


### ✨ Features ✨

* **cyanprint:** add try group subcommand [CU-86ewvp51j] ([4a43972](https://github.com/AtomiCloud/sulfone.iridium/commit/4a43972bba548ffc5b906647075e00dae0c35833))
* **cyanprint:** implement try command [CU-86ewvp51j] ([aebbe9f](https://github.com/AtomiCloud/sulfone.iridium/commit/aebbe9f6717a14619eef8ded2d82e9aeefc88499))


### 🐛 Bug Fixes 🐛

* **cyanprint:** address coderabbit local review findings [CU-86ewvp51j] ([30713f9](https://github.com/AtomiCloud/sulfone.iridium/commit/30713f9df92b6c6bc097941fd15217b5674e8493))
* **cyanprint:** address CodeRabbit review feedback [CU-86ewvp51j] ([c9dd4ab](https://github.com/AtomiCloud/sulfone.iridium/commit/c9dd4abaac663ab65e716088e41de1687b81fac7))
* **cyanprint:** address CodeRabbit review feedback [CU-86ewvp51j] ([ef32b7f](https://github.com/AtomiCloud/sulfone.iridium/commit/ef32b7f65378beee736818c51d2ac2e61b5d2eb9))
* **cyanprint:** address CodeRabbit round 3 [CU-86ewvp51j] ([e8567f0](https://github.com/AtomiCloud/sulfone.iridium/commit/e8567f05baea4a13ea0a5b44e63159ce42d29d87))

## [2.12.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v2.11.0...v2.12.0) (2026-03-11)


### 📜 Documentation 📜

* add task spec for git dirty check prompt ([6c7b86a](https://github.com/AtomiCloud/sulfone.iridium/commit/6c7b86ac655d49ec8bda068ca78c74916be1e60d))


### ✨ Features ✨

* **cyanprint:** git dirty check in update orchestrator ([b9ccc06](https://github.com/AtomiCloud/sulfone.iridium/commit/b9ccc0674b8080a807740ac75217c8eb402429ce))
* **cyanprint:** git module & CLI flag ([1828f9d](https://github.com/AtomiCloud/sulfone.iridium/commit/1828f9df16759a2c01c322ed9e573a84df3722d4))


### 🐛 Bug Fixes 🐛

* **cyanprint:** fix file deletion during upgrades [CU-86ewr17ey] ([08f65c1](https://github.com/AtomiCloud/sulfone.iridium/commit/08f65c1842e6667e635b2ad9deda56507ef60a90))
* **cyanprint:** handle all GitError variants in test [CU-86ewrj4xd] ([2a6082f](https://github.com/AtomiCloud/sulfone.iridium/commit/2a6082fe4f92cdbed0a33b4a2b4fa039ca3990d2))
* **cyancoordinator:** propagate dir-pruning errors [CU-86ewr17ey] ([c17dab5](https://github.com/AtomiCloud/sulfone.iridium/commit/c17dab57f4e9dcd887c966346896495544b22976))
* **cyanprint:** reorder cleanup before write [CU-86ewr17ey] ([1abdee1](https://github.com/AtomiCloud/sulfone.iridium/commit/1abdee159f35c254443c8a16cc0bf855656b78df))
* **cyanprint:** reorder cleanup, add tests [CU-86ewr17ey] ([2e31b37](https://github.com/AtomiCloud/sulfone.iridium/commit/2e31b37ba0ca6f10ec1b3677d353a483c1a8334d))

## [2.11.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v2.10.0...v2.11.0) (2026-03-11)


### 🧪 Tests 🧪

* **e2e:** add processor to template-resolver-1-v1 for endpoint testing ([cdbda48](https://github.com/AtomiCloud/sulfone.iridium/commit/cdbda48c24241542649c735f0a7519637280aecb))

## [2.10.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v2.9.0...v2.10.0) (2026-03-10)


### 📜 Documentation 📜

* add implementation plans for CU-86ewvp51g v1 ([6f84ef2](https://github.com/AtomiCloud/sulfone.iridium/commit/6f84ef2aae3c63a541f2fcfcea2992217a3b6348))
* add task spec for CU-86ewvp51g [Ir] Build + Push Commands ([718adc5](https://github.com/AtomiCloud/sulfone.iridium/commit/718adc5e78a18f3d28b1dd58abfe4b58d0d5c4e7))
* **cyanprint:** add v2 implementation plans [CU-86ewvp51g] ([164d1a8](https://github.com/AtomiCloud/sulfone.iridium/commit/164d1a8e37732e9ad784ab7c7c5ae5913e7d9f9b))


### ✨ Features ✨

* **cyanprint:** add --build option to push subcommands [CU-86ewvp51g] ([e7dd273](https://github.com/AtomiCloud/sulfone.iridium/commit/e7dd273999d84474fb3bec83c35fd799d6ac80e6))
* **cyanprint:** add build command with buildx [CU-86ewvp51g] ([c7e2780](https://github.com/AtomiCloud/sulfone.iridium/commit/c7e27801db8d88c996fd23b505fdc3fdda27ec04))
* **cyanprint:** add image field and --folder option [CU-86ewvp51g] ([8aa4ff5](https://github.com/AtomiCloud/sulfone.iridium/commit/8aa4ff53520ff11d7cec86e859ff13874e860e20))


### 🐛 Bug Fixes 🐛

* address coderabbit local review findings ([48573cb](https://github.com/AtomiCloud/sulfone.iridium/commit/48573cbe9535ad3bc64bf196f37066522a24688c))
* **cyanprint:** address CodeRabbit review feedback [CU-86ewvp51g] ([92457bf](https://github.com/AtomiCloud/sulfone.iridium/commit/92457bf670034544adb4a360256c3c5fbf7acab6))
* **cyanprint:** address CodeRabbit review feedback [CU-86ewvp51g] ([9225922](https://github.com/AtomiCloud/sulfone.iridium/commit/922592225d7eb3a70cf28de7cd5b228fb57d5973))
* **cyanprint:** address CodeRabbit review feedback ([2037103](https://github.com/AtomiCloud/sulfone.iridium/commit/2037103957326c06119e79ad3961c25d07ad77fe))


### 🧪 Tests 🧪

* **e2e:** update e2e fixtures to use new build format [CU-86ewvp51g] ([bcb1426](https://github.com/AtomiCloud/sulfone.iridium/commit/bcb1426264a3f5adff62c98ab6ec709fa34b7e68))

## [2.9.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v2.8.0...v2.9.0) (2026-03-09)


### 📜 Documentation 📜

* add implementation plans for CU-86ewucdfj v1 ([45fea0c](https://github.com/AtomiCloud/sulfone.iridium/commit/45fea0c9d7d6d982bfea8bb5f074939cb2935d16))
* add task spec for CU-86ewucdfj ([aa61b47](https://github.com/AtomiCloud/sulfone.iridium/commit/aa61b4788ce70b5fec67b3203d1c2b1d0848f3bb))
* replace line number references with function names in daemon docs ([5f7f35c](https://github.com/AtomiCloud/sulfone.iridium/commit/5f7f35c73f38ed48a349ee17bf75888e60708d05))


### ✨ Features ✨

* **cyanprint:** add daemon stop subcommand with cleanup ([5018e1d](https://github.com/AtomiCloud/sulfone.iridium/commit/5018e1df46552fa6f309d6e588cc12665a23b2c2))


### 🐛 Bug Fixes 🐛

* **cyanprint:** address CodeRabbit review feedback ([cb33ebd](https://github.com/AtomiCloud/sulfone.iridium/commit/cb33ebd17d7c0c7abc23dc2f0bf8b073db8557c9))
* **test:** ignore registry field in test_daemon_start_default_values ([34a0d89](https://github.com/AtomiCloud/sulfone.iridium/commit/34a0d89f4ceaa53860f355830036fcc70f554870))

## [2.8.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v2.7.0...v2.8.0) (2026-03-09)


### 📜 Documentation 📜

* **spec:** add CU-86ewrbrb0 conflict resolver specs [CU-86ewrbrb0] ([aa7027a](https://github.com/AtomiCloud/sulfone.iridium/commit/aa7027a2086662f705a4e63e64549433dd1cf626))


### ✨ Features ✨

* **cyancoordinator:** add conflict file resolver [CU-86ewrbrb0] ([4bba996](https://github.com/AtomiCloud/sulfone.iridium/commit/4bba9963580fbca9a029fd7e4a48ebb0f0ffbfeb))
* **cyanregistry:** add push_resolver() for resolvers [CU-86ewrbrb0] ([f7d836b](https://github.com/AtomiCloud/sulfone.iridium/commit/f7d836b4959b3e9d13efc6403e67cbf33d5db096))
* **e2e:** add resolver conflict test fixtures and update test templates ([e70025e](https://github.com/AtomiCloud/sulfone.iridium/commit/e70025ee53fdc4c6812ca333b013f50d4e4ad0f8))
* **cyanprint:** add resolver push command [CU-86ewrbrb0] ([97ea551](https://github.com/AtomiCloud/sulfone.iridium/commit/97ea551aea380b7d385a6e67ac79ed7dd7806360))
* **cyanregistry:** add resolvers to template push [CU-86ewrbrb0] ([c3d5f90](https://github.com/AtomiCloud/sulfone.iridium/commit/c3d5f9000332c31a955a425d783e4ce976e4bddf))
* resolver-aware e2e ([85fae6e](https://github.com/AtomiCloud/sulfone.iridium/commit/85fae6ec1c457845ca5cf04ef8a3821c022b57d0))


### 🐛 Bug Fixes 🐛

* address coderabbit local review findings [CU-86ewrbrb0] ([b5b44f0](https://github.com/AtomiCloud/sulfone.iridium/commit/b5b44f0bd71550878f2382cf69f7856be82509d0))
* **e2e:** address coderabbit review feedback [CU-86ewrbrb0] ([c253f7b](https://github.com/AtomiCloud/sulfone.iridium/commit/c253f7bd37887d01a31a697c6876fc438bf0aa89))
* **security:** address coderabbit security findings [CU-86ewrbrb0] ([e9490bc](https://github.com/AtomiCloud/sulfone.iridium/commit/e9490bc3980edd70d6dfaa45c490ac7488710d2a))
* **cyancoordinator:** fix FileOrigin to match API [CU-86ewrbrb0] ([8ea2695](https://github.com/AtomiCloud/sulfone.iridium/commit/8ea26955dd8a242d56c09f63ca2738990bd7c154))
* **cyanregistry:** fix ResolverRefReq field naming [CU-86ewrbrb0] ([5236fa2](https://github.com/AtomiCloud/sulfone.iridium/commit/5236fa24e1846e07c5250e9c6bfdba42b815acaa))
* **cyancoordinator:** ResolverOutput single not Vec [CU-86ewrbrb0] ([afc33d8](https://github.com/AtomiCloud/sulfone.iridium/commit/afc33d8fb638c1a52efcc88eb97edde5e1f5a93f))

## [2.7.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v2.6.0...v2.7.0) (2026-03-05)


### 📜 Documentation 📜

* add task spec for CU-86ewra5kn ([835d1f6](https://github.com/AtomiCloud/sulfone.iridium/commit/835d1f64850fd5d5b4bf828edb313c34182dba40))
* add v2 spec for unified batch processing [CU-86ewra5kn] ([c6c792c](https://github.com/AtomiCloud/sulfone.iridium/commit/c6c792c7380725ba5f5f132fff476837590fe6b3))
* add v5 spec and implementation plan for e2e:setup [CU-86ewra5kn] ([bf1efd1](https://github.com/AtomiCloud/sulfone.iridium/commit/bf1efd1c3f223fc802f9b709445d7116d439afd2))
* add v5 spec with post-completion feedback [CU-86ewra5kn] ([b45d65e](https://github.com/AtomiCloud/sulfone.iridium/commit/b45d65edb5d2edbae12c8d8db9d431ce4bff250f))
* fix inconsistent API documentation [CU-86ewra5kn] ([a2c12ee](https://github.com/AtomiCloud/sulfone.iridium/commit/a2c12ee1ff562611920944d70f953013d999ca15))


### ✨ Features ✨

* **e2e:** add e2e:setup task and update test fixtures [CU-86ewra5kn] ([39f4043](https://github.com/AtomiCloud/sulfone.iridium/commit/39f404382e66674072e0e14ea6964981d8392100))
* **update:** batch VFS layering for all cyan_state templates ([cd4bb84](https://github.com/AtomiCloud/sulfone.iridium/commit/cd4bb84ff908ed999c0b677efae64c424467ae69))
* prep for e2e ([e051763](https://github.com/AtomiCloud/sulfone.iridium/commit/e051763928b865b9079a7045635d4f40ddb8db7c))


### 🐛 Bug Fixes 🐛

* **coderabbit:** address local review findings [CU-86ewra5kn] ([004556d](https://github.com/AtomiCloud/sulfone.iridium/commit/004556d3546942b553680dfa6b3604d16bd47454))
* **coderabbit:** address review findings [CU-86ewra5kn] ([1bf07ff](https://github.com/AtomiCloud/sulfone.iridium/commit/1bf07ffd808f9fa8c9c70c892e375c0d0200331c))

## [2.6.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v2.5.0...v2.6.0) (2026-02-26)


### 📜 Documentation 📜

* add fast-forward branch to merge sequence diagram [CU-86ewk3qxf] ([bd666f7](https://github.com/AtomiCloud/sulfone.iridium/commit/bd666f76603fcd52c59f0fefc97ab7da6b51c5fe))
* address CodeRabbit local review findings [CU-86ewk3qxf] ([8d23bd9](https://github.com/AtomiCloud/sulfone.iridium/commit/8d23bd9c4a5ac8455f8bb716d26dc4076963aab6))
* address CodeRabbit PR review findings [CU-86ewk3qxf] ([40b792b](https://github.com/AtomiCloud/sulfone.iridium/commit/40b792b14252bf99242af246043cc6a4065ac468))
* remove reviewer-only HTML note [CU-86ewk3qxf] ([379e1ae](https://github.com/AtomiCloud/sulfone.iridium/commit/379e1ae80f215d583d6d3dd5f9bec0c4425e84a3))


### ✨ Features ✨

* coderabbit config ([84584dd](https://github.com/AtomiCloud/sulfone.iridium/commit/84584dda8e2556f34e681ca42bbadbf4d8064a0c))
* coderabbit config ([4ea51e6](https://github.com/AtomiCloud/sulfone.iridium/commit/4ea51e6b95ce486b1159bec5969523f25cb13fd2))

## [2.5.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v2.4.2...v2.5.0) (2026-02-24)


### 📜 Documentation 📜

* add task spec for CU-86ewpyvh6 ([f055045](https://github.com/AtomiCloud/sulfone.iridium/commit/f055045398a364c99f954f275781d094b8144ed9))

## [2.4.2](https://github.com/AtomiCloud/sulfone.iridium/compare/v2.4.1...v2.4.2) (2026-02-24)


### 📜 Documentation 📜

* add task spec for CU-86ewpvy6y ([de44ced](https://github.com/AtomiCloud/sulfone.iridium/commit/de44ced7f6ed0a27c06960c7837eb07316f507bc))
* update task spec to reflect actual fix [CU-86ewpvy6y] ([af1fb8b](https://github.com/AtomiCloud/sulfone.iridium/commit/af1fb8b0167222ea0e4d0b36500e1c4aba202b9b))


### 🐛 Bug Fixes 🐛

* **coord:** add explicit type annotation for exposed_ports collect [CU-86ewpvy6y] ([#63](https://github.com/AtomiCloud/sulfone.iridium/issues/63)) ([37747c0](https://github.com/AtomiCloud/sulfone.iridium/commit/37747c01247ddedbb851be0ba89c475dad64242e))
* **coord:** add explicit type annotation for exposed_ports collect ([1fa79bf](https://github.com/AtomiCloud/sulfone.iridium/commit/1fa79bfa47604611b196f4a836420e402631131f))
* **coord:** use HashMap::new() for exposed_ports inner map ([f64ff6e](https://github.com/AtomiCloud/sulfone.iridium/commit/f64ff6edb42f81d2360935644a171a6859ab3353))
* **coord:** use Vec<String> for exposed_ports field ([5990978](https://github.com/AtomiCloud/sulfone.iridium/commit/599097828261c770157038973082e15588ff5839))

## [2.4.1](https://github.com/AtomiCloud/sulfone.iridium/compare/v2.4.0...v2.4.1) (2026-02-23)


### 📜 Documentation 📜

* add Iridium Rust CLI architecture and usage documentation ([37c4cac](https://github.com/AtomiCloud/sulfone.iridium/commit/37c4cac385774d02eb8df9811552424ee07a94bd))
* add language specifiers to code blocks (MD040) ([ae39082](https://github.com/AtomiCloud/sulfone.iridium/commit/ae39082f7b5d36ead8bbba79d902e5725c11b745))
* clean up verification reports and improve CLI documentation ([629f16b](https://github.com/AtomiCloud/sulfone.iridium/commit/629f16baaa12bd61b16474e9890bd2ffd733a7f1))
* fix CodeRabbitAI review issues ([36e2b96](https://github.com/AtomiCloud/sulfone.iridium/commit/36e2b968bea914df26a82d6c2ae5ab8151adf0a7))


### 🐛 Bug Fixes 🐛

* **ci:** update macos-13 to macos-15-intel (deprecated runner) ([cd672b5](https://github.com/AtomiCloud/sulfone.iridium/commit/cd672b5129f48f452d8dc23985dc3cbcbd2b3222))

## [2.4.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v2.3.0...v2.4.0) (2025-07-01)


### ✨ Features ✨

* upgrade build and cache builds ([176c883](https://github.com/AtomiCloud/sulfone.iridium/commit/176c8835684fa45a0d88a3656ac93200afd86c51))
* upgrade build and cache builds ([#45](https://github.com/AtomiCloud/sulfone.iridium/issues/45)) ([111b676](https://github.com/AtomiCloud/sulfone.iridium/commit/111b676a10dd96f1024383935cbf8e26ab0f6b07))


### 🐛 Bug Fixes 🐛

* incorrect path to cache shell ([81f025c](https://github.com/AtomiCloud/sulfone.iridium/commit/81f025c2df76c4886cdc7b9bde51b3f31482deb9))

## [2.3.0](https://github.com/AtomiCloud/sulfone.iridium/compare/v2.2.1...v2.3.0) (2025-07-01)


### 📜 Documentation 📜

* **LLM.MD:** support for AI ([f413644](https://github.com/AtomiCloud/sulfone.iridium/commit/f4136444aeb7e80e55cf3b265e6c92cf154877b5))


### ✨ Features ✨

* allowed empty templates or groups ([ae56bbf](https://github.com/AtomiCloud/sulfone.iridium/commit/ae56bbfd726495e42c8d61c2f5423e9b12f7cc99))
* working implementation of template groups ([a31165e](https://github.com/AtomiCloud/sulfone.iridium/commit/a31165e718ff13cb08b5976bdb97fb8abcbab70b))


### 🐛 Bug Fixes 🐛

* **default:** add e2e file ([b345085](https://github.com/AtomiCloud/sulfone.iridium/commit/b3450856dfddf006720acf450da509a481ca3827))
* **default:** coordinator not passed to daemon ([11ae2f4](https://github.com/AtomiCloud/sulfone.iridium/commit/11ae2f45f8fdac546d7286595fb49f86bd8381c3))
* incorrect template for e2e ([1f4dc63](https://github.com/AtomiCloud/sulfone.iridium/commit/1f4dc632b3ae63a43e1f89c86a477724f5bdf400))

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
