# lekalo-typescript

Языковой адаптер [Lekalo](https://github.com/ichinya/lekalo) для TypeScript/JavaScript (Node): понимание исходников, типов и символов, observed-scan, bindings и native gates.

- Протокол к ядру — `lekalo.target/v1` process/JSON protocol. Реализация и сборка находятся здесь; ядро использует внешний checkout по точному commit для интеграционных проверок.
- Hono, Drizzle, Zod и PostgreSQL подключены как зависимости из отдельных репозиториев. Выбор произвольных провайдеров через конфигурацию и перенос на общий `lekalo/runtime/v1` остаются задачами [ichinya/lekalo#241](https://github.com/ichinya/lekalo/issues/241).
- Общий план отделения — [ichinya/lekalo#241](https://github.com/ichinya/lekalo/issues/241).

## Исходники и самостоятельная сборка

`adapters/node-typescript/` — runtime, compiler client, языковые генераторы и тесты. Реализации Hono, Drizzle, Zod и PostgreSQL принадлежат `lekalo-typescript-hono`, `lekalo-typescript-drizzle`, `lekalo-typescript-zod` и `lekalo-postgres`; `package-lock.json` закрепляет их Git-коммиты. Сборка включает зависимости в распространяемый runtime. Их тесты и framework fixtures находятся в репозиториях владельцев. `tests/fixtures/` и `docs/` содержат публичные данные для самостоятельных проверок; соседний checkout ядра не используется.

```sh
npm ci --prefix adapters/node-typescript --ignore-scripts --no-audit --no-fund
node adapters/node-typescript/provision-compiler.mjs
node adapters/node-typescript/build.mjs --check
node --test --test-concurrency=1 adapters/node-typescript/test/*.test.mjs
```

Lifecycle scripts намеренно запрещены: зависимости устанавливаются только с `--ignore-scripts`. Сканирование использует закреплённый native compiler; версия языка, версия runtime-пакета и версия хостового Node различаются. Runtime snapshot удалён из ядра; его development lock закрепляет внешний checkout. Регистрация провайдеров пока задана сборкой runtime: независимый выбор provider/rules через конфиг ещё не реализован.

`ci/package-workflow.yml` — неактивный шаблон CI. Для автоматического запуска его нужно установить в `.github/workflows/` учётными данными с правом `workflow`; текущая публикация проверена локально.

## Распознавание проектов для `init --adopt`

`crates/discovery` владеет языковыми правилами обнаружения, перенесёнными из
`lekalo-core/src/init/detect.rs`. Эта небольшая Rust-библиотека принимает
байты/значения; обход каталогов, no-follow чтение, проверка путей и ограничения
остаются в хосте. Команды build/test являются предложениями и не исполняются.

```sh
cargo test --locked --manifest-path crates/discovery/Cargo.toml
```

Ядро вызывает самостоятельный процесс `lekalo-typescript-discovery` по `lekalo/runtime/v1`; копий и Cargo-зависимости на этот пакет в ядре нет. Формат существующего adoption receipt сохраняется.

## Установка discovery-сервиса

```sh
cargo install --locked --path crates/discovery --root /your/tools
```

Добавьте `/your/tools/bin` в PATH, укажите `services.discovery/typescript: {command: [lekalo-typescript-discovery]}` в `lekalo.yaml` и выполните `lekalo runtime lock`. Операции: `describe`, `discovery.describe`, `discovery.inspect`; входные файлы передаются как данные, процесс не читает проект. [Контракт сервиса](https://github.com/ichinya/lekalo/blob/feat/external-services-241/docs/external-services.md). Сборка сервиса использует `debug=0`, `incremental=false`.

## Portable runtime

Select `gh/ichinya/lekalo-typescript` as a component runtime and run `lekalo runtime lock`. Node runs the pinned WebAssembly grammar. The `source.declarations/v1` result lists syntactic declarations with path, kind and name. Rule-specific `source.matches/v1` bindings under `lekalo-typescript.queries/v1` map each check ID to a Tree-sitter query. Syntax errors produce incomplete evidence. Grammar from tree-sitter-typescript 0.23.2 (see `runtime/LICENSE.grammar`).

Install the expression generator with `cargo install --locked --path crates/expressions`. Select `services.expressions/node.command: [lekalo-expressions-node]` in the project configuration, lock it, then run `lekalo expressions render FILE --target node`.
