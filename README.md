# lekalo-typescript

Языковой адаптер [Lekalo](https://github.com/ichinya/lekalo) для TypeScript/JavaScript (Node): понимание исходников, типов и символов, observed-scan, bindings и native gates.

- Протокол к ядру — lekalo.target process/JSON protocol (сейчас реализован в adapters/node-typescript монорепы).
- Framework-индексация (Vue SFC, React TSX; далее Hono/NestJS-пакеты) подключается через versioned provider API — [lekalo-php#1](https://github.com/ichinya/lekalo-php/issues/1) (контракт общий).
- Общий план отделения — [ichinya/lekalo#241](https://github.com/ichinya/lekalo/issues/241).

## Исходники и самостоятельная сборка

`adapters/node-typescript/` — исходная реализация runtime, compiler client, генераторы, Hono/Drizzle/Zod-расширения и тесты. Сохранена относительная структура, чтобы перенос не менял сгенерированные байты. `tests/fixtures/` и `docs/` содержат публичные данные для самостоятельных проверок; соседний checkout ядра не используется.

```sh
npm ci --prefix adapters/node-typescript --ignore-scripts --no-audit --no-fund
node adapters/node-typescript/provision-compiler.mjs
node adapters/node-typescript/build.mjs --check
node --test --test-concurrency=1 adapters/node-typescript/test/*.test.mjs
```

Lifecycle scripts намеренно запрещены: зависимости устанавливаются только с `--ignore-scripts`. Сканирование использует закреплённый native compiler; версия языка, версия runtime-пакета и версия хостового Node различаются. Основной репозиторий сохраняет проверяемый по commit+SHA-256 snapshot для переходной поставки и host-conformance. Изменения реализации вносятся сюда, затем обновляется snapshot.

`ci/package-workflow.yml` — неактивный шаблон CI. Для автоматического запуска его нужно установить в `.github/workflows/` учётными данными с правом `workflow`; текущая публикация проверена локально.
