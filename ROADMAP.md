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

## Segurança

- [x] Auditoria CWE/CVSS do SDK (14 findings identificados)
- [x] Correção de off-by-one em Args::get() (CWE-125)
- [x] Validação de ponteiro nulo em get_ref() (CWE-416)
- [x] Proteção contra integer overflow em allot() (CWE-190)
- [x] Bounds checking em packed string parsing (CWE-125)
- [x] Ordering correto em AtomicPtr (CWE-362)
- [x] Validação de ponteiros em Ref::new() e from_table() (CWE-476/843)
- [x] Proteção contra OOM em alocação de strings (CWE-20)
- [x] Validação em release() e count() (CWE-763/190)
- [x] Testes de segurança para args, encoding e error
- [x] Adicionar cargo audit ao CI
- [x] Eliminar UB em Runtime via UnsafeCell (CWE-362)
- [x] Leak explícito de nomes de natives com Box::leak (CWE-401)
- [x] Refinamento de bounds check em packed string (CWE-125)
- [x] Validação de tamanho em into_sized_buffer() (CWE-120)
