# Система контроля версий

VCS разделена на 2 основных части: одна реализует непосредственно CLI с валидацией пользовательского ввода,
вторая — библиотека, абстрагирующая пользовательский интерфейс от работы с локальным и удалённым хранилищем.

Репозиторий — это дерево из коммитов. Каждый коммит содержит ссылку на дерево, которое хранит в себе
информацию о содержимом объектов/файлов в репозитории. Объекты хранятся в сжатом виде и адресуются по хэшу
содержимого. Хранилище по хэшу можно абстрагировать и реализовать как в виде файлов в локальной
ФС, так и в удалённом persistent key-value хранилище (или S3-like).

## Диаграмма классов
```mermaid
classDiagram
    class Repository {
        +id: RepoId
        +open(path)
        +commit(message, author)
        +checkout(ref)
        +merge(branch)
        +clone()
        +fetch()
        +pull()
    }


    class Commit {
        +hash: Hash
        +author: Author
        +date: DateTime
        +message: string
        +tree: Tree
        +parents: List~Commit~
    }


    class Tree {
        +entries: Map~Path, Blob~
    }


    class Blob {
        +hash: Hash
        +content: bytes (compressed)
    }


    class Branch {
        +name: string
        +head: Commit
    }

    class Index {
        +add(file)
        +remove(file)
        +clear()
    }

    class ObjectStore {
        +store(obj)
        +load(hash)
    }

    class RefStore {
        +getHead()
        +setHead(commit)
        +createBranch(name, commit)
        +deleteBranch(name)
    }

    class MergeEngine {
        +merge(base, ours, theirs)
    }

    class DiffEngine {
        +diff(a, b)
    }


    Repository --> Commit
    Commit --> Tree
    Tree --> Blob
    Repository --> Branch
    Repository --> Index
    Repository --> ObjectStore
    Repository --> RefStore
    Repository --> MergeEngine
    MergeEngine --> DiffEngine
```


## Диаграммма компонентов
```mermaid
flowchart LR
    CLI[CLI Interface]
    API[VCS Library]


    subgraph Core[VCS Core]
        Repo[Repository]
        ObjStore[Object Store]
        RefStore[Refs & Branches]
        Index[Index / Staging]
        History[History & Log]
        Merge[Merge Engine]
        Diff[Diff Engine]
        FS[Filesystem Abstraction]
    end


    subgraph Remote[Remote subsystem]
        Client[Remote Client]
        Proto[Protocol Layer]
        ServerAPI[Server API]
    end


    CLI --> API
    API --> Repo


    Repo --> ObjStore
    Repo --> RefStore
    Repo --> Index
    Repo --> History
    Repo --> Merge


    Merge --> Diff
    ObjStore --> FS
    RefStore --> FS
    Index --> FS


    API --> Client
    Client --> Proto
    Proto --> ServerAPI
```
