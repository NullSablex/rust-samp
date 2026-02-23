# Roadmap

Metas e objetivos para o desenvolvimento do rust-samp.

## CI/CD

- [x] Adicionar build Windows i686 no GitHub Actions
- [x] Adicionar build Linux i686 no GitHub Actions
- [x] Configurar cache de dependências cross-platform

## Documentação

- [x] Definir formato (mdBook ou Wiki do GitHub)
- [x] Guia de introdução: como criar um plugin do zero
- [x] Referência da API: tipos, traits e macros principais
- [x] Guia de encoding: como usar Windows-1251/1252
- [x] Exemplos comentados além do plugin-example
- [x] Documentar funções e módulos públicos do samp-sdk

## Tratamento de Erros

- [x] Revisar blocos `unsafe` que não validam ponteiros
- [x] Melhorar mensagens de erro nas macros procedurais
- [x] Avaliar uso de `Result` em mais pontos da API pública

## Testes

- [x] Validação em produção (em andamento)
- [x] Avaliar viabilidade de mock do AMX para testes unitários
- [x] **[Longo prazo]** Implementar testes unitários para componentes isoláveis
