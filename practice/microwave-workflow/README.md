# Диаграмма конечных автоматов работы микроволновой печи
![State diagram](https://raw.github.com//StackOverflow/master/question.13808020.include-an-svg-hosted-on-github-in-markdown/controllers_brief.svg?sanitize=true)

```mermaid
stateDiagram-v2
    [*] --> Ожидание
    note right of Ожидание
        Готовность, дисплей 0:00
        Дверца закрыта
    end note

    Ожидание --> Дверца_Открыта: открыть_дверцу
    Дверца_Открыта --> Выбор_Режима: закрыть_дверцу / поместил_еду
    Дверца_Открыта --> Ожидание: [Программа завершена, дверь закрыта]
    Дверца_Открыта --> Ошибка: показать_ошибку()

    Выбор_Режима --> Ожидание: отмена / сброс_настроек
    Выбор_Режима --> Готовка: старт [режим_выбран & время>0] / запустить_таймер()
    Выбор_Режима --> Дверца_Открыта: открыть_дверцу

    Готовка --> Пауза: пауза() & выключить_магнетрон()
    Готовка --> Пауза: открыть_дверцу & выключить_магнетрон()
    Готовка --> Завершено: таймер = 0 & сигнал()
    Готовка --> Ошибка: выключить_магнетрон() & показать_ошибку()

    Пауза --> Готовка: продолжить [дверца_закрыта & время>0] / запустить_таймер()
    Пауза --> Дверца_Открыта: открыть_дверцу
    Пауза --> Ошибка: показать_ошибку()

    Дверца_Открыта --> Пауза: закрыть_дверцу [была_пауза]
    Дверца_Открыта --> Ошибка: показать_ошибку()

    Завершено --> Ожидание: сброс / подтверждение
    Завершено --> Дверца_Открыта: открыть_дверцу

    Ожидание --> Ошибка: показать_ошибку()
    Ошибка --> Ожидание: сброс_ошибки / очистить()
```

# Временная диаграмма готовки в микроволновой печи
[![Timing diagram](https://img.plantuml.biz/plantuml/svg/dL9D2y8m3BtdLnH_WCmFK6-5UV4W4_NahMnHXbqorixVxr9r7O8AtfANz_9UGxNpfl5jPS7bngAQZXE0d2al7QhHchaPAUEK517ERaZgu8t7C0nLd6xDXT0tJ67OTv5mB2jyh0qLW4v035JfL6hPOG5XfVIRmYBqEmJ9pcS8EcbEs72ddyG5MOqKvO861FJqXmdeMHAX0tfVqaMCebi7ThJvW9OoNplQMqAAadrB90JGYu5iPre9yFbTJmBevUVKFMVrNw39bukaNDKM_TUChziFp_Qy958XN-eE)](https://editor.plantuml.com/uml/dL9D2y8m3BtdLnH_WCmFK6-5UV4W4_NahMnHXbqorixVxr9r7O8AtfANz_9UGxNpfl5jPS7bngAQZXE0d2al7QhHchaPAUEK517ERaZgu8t7C0nLd6xDXT0tJ67OTv5mB2jyh0qLW4v035JfL6hPOG5XfVIRmYBqEmJ9pcS8EcbEs72ddyG5MOqKvO861FJqXmdeMHAX0tfVqaMCebi7ThJvW9OoNplQMqAAadrB90JGYu5iPre9yFbTJmBevUVKFMVrNw39bukaNDKM_TUChziFp_Qy958XN-eE)
