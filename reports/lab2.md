# lab2-李国玮

## 问答作业

### 1

![../_images/sv39-pte.png](https://learningos.cn/rCore-Tutorial-Guide-2023A/_images/sv39-pte.png)

63-54位：保留

53-10位：物理页号，分为3级页表

9-8位：RSW，Reserved for Software。 用于预留给软件做自定义页表功能的位。

D：dirty，表示该页表项对应的虚拟页表时候被修改过

A：access，虚拟页面是否被访问过

G：global，如果为1,则所有页表都包含这一项

U：该页表项对应的虚拟页面是否允许U特权级访问

RWX：该虚拟页面是否允许读、写、可执行

V：valid，只有为1时才合法。