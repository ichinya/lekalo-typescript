# lekalo-typescript

Языковой адаптер [Lekalo](https://github.com/ichinya/lekalo) для TypeScript/JavaScript (Node): понимание исходников, типов и символов, observed-scan, bindings и native gates.

- Протокол к ядру — lekalo.target process/JSON protocol (сейчас реализован в adapters/node-typescript монорепы).
- Framework-индексация (Vue SFC, React TSX; далее Hono/NestJS-пакеты) подключается через versioned provider API — [lekalo-php#1](https://github.com/ichinya/lekalo-php/issues/1) (контракт общий).
- Общий план отделения — [ichinya/lekalo#241](https://github.com/ichinya/lekalo/issues/241).
